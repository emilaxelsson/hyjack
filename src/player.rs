use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use midir::{MidiOutput, MidiOutputConnection};

use crate::state::AppState;
use crate::state::pattern::Pattern;

pub const BPM: u64 = 120;
const MIDI_CHANNEL: u8 = 0;

pub trait MidiSink {
  fn send(&mut self, message: &[u8]) -> Result<()>;
}

pub struct SilentMidiSink;

impl MidiSink for SilentMidiSink {
  fn send(&mut self, _message: &[u8]) -> Result<()> {
    Ok(())
  }
}

pub struct SystemMidiSink(MidiOutputConnection);

impl MidiSink for SystemMidiSink {
  fn send(&mut self, message: &[u8]) -> Result<()> {
    self.0.send(message)?;
    Ok(())
  }
}

pub fn select_midi_sink() -> Result<Box<dyn MidiSink>> {
  let midi_output = MidiOutput::new("Dracker")?;
  let ports = midi_output.ports();
  if ports.is_empty() {
    println!("No MIDI output ports found. Playback will be silent.");
    return Ok(Box::new(SilentMidiSink));
  }

  println!("MIDI output:");
  println!("  0: Silent playback");
  for (index, port) in ports.iter().enumerate() {
    println!("  {}: {}", index + 1, midi_output.port_name(port)?);
  }

  loop {
    print!("Select output: ");
    io::stdout().flush()?;
    let mut selection = String::new();
    io::stdin().read_line(&mut selection)?;
    match selection.trim().parse::<usize>() {
      Ok(0) => return Ok(Box::new(SilentMidiSink)),
      Ok(index) if index <= ports.len() => {
        let connection = midi_output
          .connect(&ports[index - 1], "Dracker output")
          .map_err(|_| anyhow::anyhow!("Could not connect to MIDI output"))?;
        return Ok(Box::new(SystemMidiSink(connection)));
      }
      _ => println!("Enter a number from 0 to {}.", ports.len()),
    }
  }
}

pub struct Playback {
  next_row_at: Instant,
  active_notes: Vec<u8>,
  last_program: Option<u8>,
}

#[derive(Default)]
pub struct Player {
  playback: Option<Playback>,
}

impl Player {
  pub fn start(
    &mut self,
    state: &mut AppState,
    midi: &mut dyn MidiSink,
    now: Instant,
  ) -> Result<()> {
    if self.playback.is_some() {
      bail!("Player is already running");
    }
    self.playback = Some(Playback::start(state, midi, now)?);
    Ok(())
  }

  pub fn next_deadline(&self) -> Option<Instant> {
    self.playback.as_ref().map(Playback::next_deadline)
  }

  pub fn tick(
    &mut self,
    state: &mut AppState,
    midi: &mut dyn MidiSink,
    now: Instant,
  ) -> Result<()> {
    self
      .playback
      .as_mut()
      .ok_or_else(|| anyhow::anyhow!("Player is not running"))?
      .tick(state, midi, now)
  }

  pub fn stop(&mut self, midi: &mut dyn MidiSink) -> Result<()> {
    if let Some(mut playback) = self.playback.take() {
      playback.stop(midi)?;
    }
    Ok(())
  }
}

impl Playback {
  pub fn start(state: &mut AppState, midi: &mut dyn MidiSink, now: Instant) -> Result<Self> {
    normalize_position(state)?;
    let mut playback = Self {
      next_row_at: now + row_duration(current_pattern(state)?),
      active_notes: Vec::new(),
      last_program: None,
    };
    playback.play_row(state, midi)?;
    Ok(playback)
  }

  pub fn next_deadline(&self) -> Instant {
    self.next_row_at
  }

  pub fn tick(
    &mut self,
    state: &mut AppState,
    midi: &mut dyn MidiSink,
    now: Instant,
  ) -> Result<()> {
    while now >= self.next_row_at {
      let duration = row_duration(current_pattern(state)?);
      advance_position(state)?;
      advance_time(state, duration);
      self.next_row_at += duration;
      self.play_row(state, midi)?;
    }
    Ok(())
  }

  pub fn stop(&mut self, midi: &mut dyn MidiSink) -> Result<()> {
    self.release_notes(midi)
  }

  fn play_row(&mut self, state: &AppState, midi: &mut dyn MidiSink) -> Result<()> {
    self.release_notes(midi)?;
    let row_index = state.player.row_index;
    let notes: Vec<_> = current_pattern(state)?
      .tracks
      .iter()
      .filter_map(|track| track.notes[row_index].as_ref())
      .collect();

    for note in notes {
      let program = note.instrument.0;
      if self.last_program != Some(program) {
        midi.send(&[0xc0 | MIDI_CHANNEL, program])?;
        self.last_program = Some(program);
      }

      let pitch = crate::state::pattern::midi_pitch_from_octave(note.octave, note.semitone);
      midi.send(&[0x90 | MIDI_CHANNEL, pitch, note.velocity.0])?;
      self.active_notes.push(pitch);
    }
    Ok(())
  }

  fn release_notes(&mut self, midi: &mut dyn MidiSink) -> Result<()> {
    for pitch in self.active_notes.drain(..) {
      midi.send(&[0x80 | MIDI_CHANNEL, pitch, 0])?;
    }
    Ok(())
  }
}

pub fn row_duration(pattern: &Pattern) -> Duration {
  Duration::from_secs(60).div_f64(BPM as f64 * f64::from(pattern.rows_per_beat.get()))
}

fn current_pattern(state: &AppState) -> Result<&Pattern> {
  let title = state
    .sequence
    .titles
    .get(state.player.sequence_index)
    .ok_or_else(|| anyhow::anyhow!("Player sequence index is out of bounds"))?;
  state
    .patterns
    .iter()
    .find(|pattern| pattern.title == *title)
    .ok_or_else(|| anyhow::anyhow!("Pattern not found: {title}"))
}

fn normalize_position(state: &mut AppState) -> Result<()> {
  if state.sequence.titles.is_empty() {
    bail!("Cannot play an empty sequence");
  }
  let last_song_index = state
    .sequence
    .end_of_song
    .min(state.sequence.titles.len() - 1);
  state.player.sequence_index = state.player.sequence_index.min(last_song_index);
  let num_rows = current_pattern(state)?.num_rows.get();
  state.player.row_index = state.player.row_index.min(num_rows - 1);
  Ok(())
}

fn advance_position(state: &mut AppState) -> Result<()> {
  let num_rows = current_pattern(state)?.num_rows.get();
  if state.player.row_index + 1 < num_rows {
    state.player.row_index += 1;
  } else {
    let last_song_index = state
      .sequence
      .end_of_song
      .min(state.sequence.titles.len() - 1);
    state.player.sequence_index = if state.player.sequence_index >= last_song_index {
      0
    } else {
      state.player.sequence_index + 1
    };
    state.player.row_index = 0;
  }
  Ok(())
}

fn advance_time(state: &mut AppState, duration: Duration) {
  let current_millis = u64::from(state.player.time.hour) * 3_600_000
    + u64::from(state.player.time.minute) * 60_000
    + u64::from(state.player.time.second) * 1_000
    + u64::from(state.player.time.millis);
  let total_millis = current_millis + duration.as_millis() as u64;
  state.player.time.hour = ((total_millis / 3_600_000) % 256) as u8;
  state.player.time.minute = ((total_millis / 60_000) % 60) as u8;
  state.player.time.second = ((total_millis / 1_000) % 60) as u8;
  state.player.time.millis = (total_millis % 1_000) as u16;
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::instruments::InstrumentIndex;
  use crate::state::pattern::{Note, Octave, Semitone, Velocity};

  struct RecordingMidiSink(Vec<Vec<u8>>);

  impl MidiSink for RecordingMidiSink {
    fn send(&mut self, message: &[u8]) -> Result<()> {
      self.0.push(message.to_vec());
      Ok(())
    }
  }

  #[test]
  fn starts_with_program_changes_and_notes_in_track_order() {
    let mut state: AppState = serde_json::from_str(include_str!("../songs/test.json")).unwrap();
    state.player.sequence_index = 1;
    state.player.row_index = 0;
    let mut midi = RecordingMidiSink(Vec::new());

    Playback::start(&mut state, &mut midi, Instant::now()).unwrap();

    assert_eq!(midi.0, vec![vec![0xc0, 2], vec![0x90, 36, 20]]);
  }

  #[test]
  fn player_start_and_stop_manage_the_active_playback() {
    let mut state: AppState = serde_json::from_str(include_str!("../songs/test.json")).unwrap();
    state.player.sequence_index = 1;
    state.player.row_index = 0;
    let mut midi = RecordingMidiSink(Vec::new());
    let mut player = Player::default();

    assert!(player.next_deadline().is_none());
    player.start(&mut state, &mut midi, Instant::now()).unwrap();
    assert!(player.next_deadline().is_some());
    player.stop(&mut midi).unwrap();

    assert!(player.next_deadline().is_none());
    assert_eq!(midi.0.last(), Some(&vec![0x80, 36, 0]));
  }

  #[test]
  fn linearizes_simultaneous_notes_with_program_changes() {
    let mut state: AppState = serde_json::from_str(include_str!("../songs/test.json")).unwrap();
    state.player.sequence_index = 1;
    state.player.row_index = 0;
    let pattern = state
      .patterns
      .iter_mut()
      .find(|pattern| pattern.title == "Intro B")
      .unwrap();
    pattern.tracks[1].notes[0] = Some(Note {
      octave: Octave(3),
      semitone: Semitone(4),
      instrument: InstrumentIndex(4),
      velocity: Velocity(80),
    });
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&mut state, &mut midi, now).unwrap();

    assert_eq!(
      midi.0,
      vec![
        vec![0xc0, 2],
        vec![0x90, 36, 20],
        vec![0xc0, 4],
        vec![0x90, 52, 80]
      ]
    );

    playback
      .tick(&mut state, &mut midi, now + Duration::from_millis(125))
      .unwrap();
    assert_eq!(midi.0[4..], [vec![0x80, 36, 0], vec![0x80, 52, 0]]);
  }

  #[test]
  fn loops_after_the_last_song_pattern() {
    let mut state: AppState = serde_json::from_str(include_str!("../songs/test.json")).unwrap();
    state.sequence.end_of_song = 0;
    state.player.sequence_index = 0;
    state.player.row_index = 7;
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&mut state, &mut midi, now).unwrap();

    playback
      .tick(&mut state, &mut midi, now + Duration::from_millis(125))
      .unwrap();

    assert_eq!(state.player.sequence_index, 0);
    assert_eq!(state.player.row_index, 0);
  }
}
