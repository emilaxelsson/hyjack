use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io::{self, Write};
use std::num::NonZeroU8;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use midir::{MidiOutput, MidiOutputConnection};

use crate::state::AppState;
use crate::state::pattern::{Effect, Note, Pattern, midi_pitch_from_octave};

const MIDI_CHANNEL: u8 = 0;
pub const MIDI_PROGRAM_CHANGE: u8 = 0xc0;
pub const MIDI_NOTE_ON: u8 = 0x90;

#[derive(Default)]
pub struct PlayerState {
  pub time: Duration,
  pub sequence_index: usize,
  pub row_index: usize,
}

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
  let midi_output = MidiOutput::new("Hyjack")?;
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
          .connect(&ports[index - 1], "Hyjack output")
          .map_err(|_| anyhow::anyhow!("Could not connect to MIDI output"))?;
        return Ok(Box::new(SystemMidiSink(connection)));
      }
      _ => println!("Enter a number from 0 to {}.", ports.len()),
    }
  }
}

pub struct Playback {
  start_at: Instant,
  start_time: i128,
  next_row_at: Instant,
  timeline: Timeline,
  /// Priority queue containing the next occurrence of each timeline note.
  ///
  /// `Reverse` puts the earliest event first, including early notes from a
  /// subsequent loop. After dispatch, the next occurrence is inserted.
  pending_notes: BinaryHeap<Reverse<ScheduledNote>>,
  last_program: Option<u8>,
}

struct TimelineRow {
  start_time: Duration,
  duration: Duration,
}

#[derive(Debug, PartialEq, Eq)]
struct NoteEvent {
  /// Signed nanoseconds from the song start. An early note can have a negative
  /// time when its delay places it before the first row boundary.
  time: i128,
  sequence_index: usize,
  row_index: usize,
  track_index: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ScheduledNote {
  time: i128,
  track_index: usize,
  /// Identifies the song traversal for this occurrence: zero is the initial
  /// traversal, and each subsequent loop increments it. Used to schedule the
  /// next occurrence without duplicating notes across loop boundaries.
  cycle: u64,
  sequence_index: usize,
  row_index: usize,
  event_index: usize,
}

/// Cached timing data for every row in each playable sequence entry.
struct Timeline {
  /// Cached row timing data. The outer index is the sequence index and the inner index is
  /// the row index.
  rows: Vec<Vec<TimelineRow>>,
  notes: Vec<NoteEvent>,
  duration: Duration,
}

#[derive(Default)]
pub struct Player {
  state: PlayerState,
  playback: Option<Playback>,
}

impl Player {
  pub fn state(&self) -> &PlayerState {
    &self.state
  }

  pub fn state_mut(&mut self) -> &mut PlayerState {
    &mut self.state
  }

  pub fn emit_note_on(&self, note: &Note, midi: &mut dyn MidiSink) -> Result<()> {
    let pitch = midi_pitch_from_octave(note.octave, note.semitone);
    midi.send(&[MIDI_PROGRAM_CHANGE | MIDI_CHANNEL, note.instrument.0])?;
    midi.send(&[MIDI_NOTE_ON | MIDI_CHANNEL, pitch, note.midi_velocity()])
  }

  pub fn start(&mut self, song: &AppState, midi: &mut dyn MidiSink, now: Instant) -> Result<()> {
    if self.playback.is_some() {
      bail!("Player is already running");
    }
    self.playback = Some(Playback::start(song, &mut self.state, midi, now)?);
    Ok(())
  }

  pub fn next_deadline(&self) -> Option<Instant> {
    self.playback.as_ref().map(Playback::next_deadline)
  }

  pub fn tick(&mut self, song: &AppState, midi: &mut dyn MidiSink, now: Instant) -> Result<()> {
    self
      .playback
      .as_mut()
      .ok_or_else(|| anyhow::anyhow!("Player is not running"))?
      .tick(song, &mut self.state, midi, now)
  }

  pub fn stop(&mut self) {
    self.playback = None;
  }
}

impl Playback {
  pub fn start(
    song: &AppState,
    player: &mut PlayerState,
    midi: &mut dyn MidiSink,
    now: Instant,
  ) -> Result<Self> {
    normalize_position(song, player);
    let timeline = Timeline::from_song(song)?;
    player.time = timeline.row(player)?.start_time;
    let start_time = Timeline::duration_nanos(player.time);
    let mut playback = Self {
      start_at: now,
      start_time,
      next_row_at: now + timeline.row(player)?.duration,
      timeline,
      pending_notes: BinaryHeap::new(),
      last_program: None,
    };
    for (event_index, event) in playback.timeline.notes.iter().enumerate() {
      let initial =
        (event.sequence_index, event.row_index) >= (player.sequence_index, player.row_index);
      let cycle = if initial { 0 } else { 1 };
      playback
        .pending_notes
        .push(Reverse(playback.schedule_note(event_index, cycle, initial)));
    }
    playback.tick(song, player, midi, now)?;
    Ok(playback)
  }

  pub fn next_deadline(&self) -> Instant {
    self
      .note_deadline()
      .map_or(self.next_row_at, |note_at| self.next_row_at.min(note_at))
  }

  pub fn tick(
    &mut self,
    song: &AppState,
    player: &mut PlayerState,
    midi: &mut dyn MidiSink,
    now: Instant,
  ) -> Result<()> {
    while now >= self.next_deadline() {
      let note_at = self.note_deadline();
      // Row boundaries take precedence over notes at the same instant.
      if note_at.is_none_or(|note_at| self.next_row_at <= note_at) {
        advance_position(song, player)?;
        let row = self.timeline.row(player)?;
        player.time = row.start_time;
        self.next_row_at += row.duration;
      } else {
        let scheduled = &self
          .pending_notes
          .peek()
          .expect("note deadline has an event")
          .0;
        let event = &self.timeline.notes[scheduled.event_index];
        let note = pattern_at(song, event.sequence_index)?
          .tracks
          .get(event.track_index)
          .and_then(|track| track.notes.get(event.row_index))
          .and_then(Option::as_ref)
          .ok_or_else(|| anyhow::anyhow!("Scheduled note is missing from the song"))?;
        let event_index = scheduled.event_index;
        let next_cycle = scheduled.cycle + 1;
        self.play_note(note, midi)?;
        self.pending_notes.pop();
        self
          .pending_notes
          .push(Reverse(self.schedule_note(event_index, next_cycle, false)));
      }
    }
    Ok(())
  }

  fn schedule_note(&self, event_index: usize, mut cycle: u64, clamp: bool) -> ScheduledNote {
    let event = &self.timeline.notes[event_index];
    let loop_duration = Timeline::duration_nanos(self.timeline.duration);
    let mut time = event.time + i128::from(cycle) * loop_duration;
    if clamp {
      time = time.max(self.start_time);
    } else {
      while time < self.start_time {
        cycle += 1;
        time += loop_duration;
      }
    }
    ScheduledNote {
      time,
      track_index: event.track_index,
      cycle,
      sequence_index: event.sequence_index,
      row_index: event.row_index,
      event_index,
    }
  }

  fn note_deadline(&self) -> Option<Instant> {
    self.pending_notes.peek().map(|scheduled| {
      let nanos = scheduled.0.time - self.start_time;
      let seconds = u64::try_from(nanos / 1_000_000_000)
        .expect("scheduled note deadline must fit in a duration");
      let subsec_nanos = u32::try_from(nanos % 1_000_000_000)
        .expect("scheduled note deadline must not precede playback start");
      self.start_at + Duration::new(seconds, subsec_nanos)
    })
  }

  fn play_note(&mut self, note: &Note, midi: &mut dyn MidiSink) -> Result<()> {
    let program = note.instrument.0;
    if self.last_program != Some(program) {
      midi.send(&[MIDI_PROGRAM_CHANGE | MIDI_CHANNEL, program])?;
      self.last_program = Some(program);
    }

    let pitch = midi_pitch_from_octave(note.octave, note.semitone);
    midi.send(&[MIDI_NOTE_ON | MIDI_CHANNEL, pitch, note.midi_velocity()])
  }
}

fn row_duration(bpm: NonZeroU8, rows_per_beat: NonZeroU8) -> Duration {
  Duration::from_secs(60).div_f64(bpm.get() as f64 * f64::from(rows_per_beat.get()))
}

fn pattern_at(song: &AppState, sequence_index: usize) -> Result<&Pattern> {
  let title = song
    .sequence
    .titles
    .get(sequence_index)
    .ok_or_else(|| anyhow::anyhow!("Player sequence index is out of bounds"))?;
  song
    .patterns
    .iter()
    .find(|pattern| pattern.title == *title)
    .ok_or_else(|| anyhow::anyhow!("Pattern not found: {title}"))
}

fn current_pattern<'a>(song: &'a AppState, player: &PlayerState) -> Result<&'a Pattern> {
  pattern_at(song, player.sequence_index)
}

impl Timeline {
  /// Builds row boundaries and note events for every playable sequence entry.
  ///
  /// Effects are applied in track order before the affected row's duration is calculated, so
  /// later tracks override earlier effects in the same row. Returns an error if a playable
  /// sequence entry does not name a pattern.
  fn from_song(song: &AppState) -> Result<Self> {
    let last_song_index = song
      .sequence
      .end_of_song
      .min(song.sequence.titles.len() - 1);
    let mut bpm = song.initial_bpm;
    let mut start_time = Duration::ZERO;
    let mut rows = Vec::with_capacity(last_song_index + 1);
    let mut notes = Vec::new();

    for sequence_index in 0..=last_song_index {
      let pattern = pattern_at(song, sequence_index)?;
      let mut pattern_rows = Vec::with_capacity(pattern.num_rows.get());

      for row_index in 0..pattern.num_rows.get() {
        for track in &pattern.tracks {
          if let Some(Effect::SetTempo(new_bpm)) = track
            .notes
            .get(row_index)
            .and_then(Option::as_ref)
            .and_then(|note| note.effect)
          {
            bpm = new_bpm;
          }
        }

        let duration = row_duration(bpm, pattern.rows_per_beat);
        for (track_index, track) in pattern.tracks.iter().enumerate() {
          if let Some(note) = track.notes.get(row_index).and_then(Option::as_ref) {
            let delay = match note.effect {
              Some(Effect::Delay(delay)) => delay.get(),
              _ => 0,
            };
            notes.push(NoteEvent {
              time: Self::duration_nanos(start_time)
                + Self::duration_nanos(duration) * i128::from(delay) / 100,
              sequence_index,
              row_index,
              track_index,
            });
          }
        }
        pattern_rows.push(TimelineRow {
          start_time,
          duration,
        });
        start_time += duration;
      }

      rows.push(pattern_rows);
    }

    notes.sort_by_key(|event| {
      (
        event.time,
        event.sequence_index,
        event.row_index,
        event.track_index,
      )
    });

    Ok(Self {
      rows,
      notes,
      duration: start_time,
    })
  }

  fn duration_nanos(duration: Duration) -> i128 {
    i128::from(duration.as_secs()) * 1_000_000_000 + i128::from(duration.subsec_nanos())
  }

  fn row(&self, player: &PlayerState) -> Result<&TimelineRow> {
    self
      .rows
      .get(player.sequence_index)
      .and_then(|rows| rows.get(player.row_index))
      .ok_or_else(|| anyhow::anyhow!("Player position is out of bounds for playback timeline"))
  }
}

/// Clamps the playback cursor to the playable sequence and row ranges.
///
/// Resets the cursor to the beginning when the selected pattern cannot be found.
fn normalize_position(song: &AppState, player: &mut PlayerState) {
  let last_song_index = song
    .sequence
    .end_of_song
    .min(song.sequence.titles.len() - 1);
  player.sequence_index = player.sequence_index.min(last_song_index);
  match current_pattern(song, player) {
    Ok(pattern) => {
      player.row_index = player.row_index.min(pattern.num_rows.get() - 1);
    }
    Err(_) => {
      player.sequence_index = 0;
      player.row_index = 0;
    }
  }
}

fn advance_position(song: &AppState, player: &mut PlayerState) -> Result<()> {
  let num_rows = current_pattern(song, player)?.num_rows.get();
  if player.row_index + 1 < num_rows {
    player.row_index += 1;
  } else {
    let last_song_index = song
      .sequence
      .end_of_song
      .min(song.sequence.titles.len() - 1);
    player.sequence_index = if player.sequence_index >= last_song_index {
      0
    } else {
      player.sequence_index + 1
    };
    player.row_index = 0;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::instruments::InstrumentIndex;
  use crate::state::minimal_song;
  use crate::state::pattern::{Delay, Note, Octave, Semitone, Track, Velocity};

  struct RecordingMidiSink(Vec<Vec<u8>>);

  impl MidiSink for RecordingMidiSink {
    fn send(&mut self, message: &[u8]) -> Result<()> {
      self.0.push(message.to_vec());
      Ok(())
    }
  }

  fn test_note(effect: Option<Effect>) -> Note {
    Note {
      octave: Octave(3),
      semitone: Semitone(0),
      instrument: InstrumentIndex(0),
      velocity: Velocity(50),
      effect,
    }
  }

  fn pitched_note(semitone: u8, instrument: u8, delay: Option<i8>) -> Note {
    let mut note = test_note(delay.map(|value| Effect::Delay(Delay::new(value).unwrap())));
    note.semitone = Semitone(semitone);
    note.instrument = InstrumentIndex(instrument);
    note
  }

  #[test]
  fn playback_dispatches_notes_at_deadlines_without_advancing_the_row() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(2).unwrap();
    state.patterns[0].tracks = vec![
      Track {
        notes: vec![
          Some(pitched_note(0, 1, None)),
          Some(pitched_note(1, 1, Some(30))),
        ],
      },
      Track {
        notes: vec![None, Some(pitched_note(2, 2, Some(-90)))],
      },
      Track {
        notes: vec![
          Some(pitched_note(3, 3, Some(90))),
          Some(pitched_note(4, 3, Some(0))),
        ],
      },
    ];
    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 1], vec![MIDI_NOTE_ON, 36, 64]]
    );
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(12_500))
    );

    player
      .tick(&state, &mut midi, now + Duration::from_micros(12_499))
      .unwrap();
    assert_eq!(midi.0.len(), 2);
    player
      .tick(&state, &mut midi, now + Duration::from_micros(12_500))
      .unwrap();
    assert_eq!(player.state().row_index, 0);
    assert_eq!(player.state().time, Duration::ZERO);
    assert_eq!(
      &midi.0[2..],
      &[vec![MIDI_PROGRAM_CHANGE, 2], vec![MIDI_NOTE_ON, 38, 64]]
    );
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(112_500))
    );

    player
      .tick(&state, &mut midi, now + Duration::from_micros(112_500))
      .unwrap();
    assert_eq!(
      &midi.0[4..],
      &[vec![MIDI_PROGRAM_CHANGE, 3], vec![MIDI_NOTE_ON, 39, 64]]
    );
    assert_eq!(player.state().row_index, 0);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(125))
    );

    player
      .tick(&state, &mut midi, now + Duration::from_millis(125))
      .unwrap();
    assert_eq!(player.state().row_index, 1);
    assert_eq!(player.state().time, Duration::from_millis(125));
    assert_eq!(&midi.0[6..], &[vec![MIDI_NOTE_ON, 40, 64]]);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(162_500))
    );

    player
      .tick(&state, &mut midi, now + Duration::from_micros(162_500))
      .unwrap();
    assert_eq!(
      &midi.0[7..],
      &[vec![MIDI_PROGRAM_CHANGE, 1], vec![MIDI_NOTE_ON, 37, 64]]
    );
  }

  #[test]
  fn late_ticks_process_all_events_in_time_and_track_order() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(2).unwrap();
    state.patterns[0].tracks = vec![
      Track {
        notes: vec![
          Some(pitched_note(0, 1, Some(30))),
          Some(pitched_note(1, 1, None)),
        ],
      },
      Track {
        notes: vec![
          Some(pitched_note(2, 2, Some(30))),
          Some(pitched_note(3, 2, None)),
        ],
      },
      Track {
        notes: vec![Some(pitched_note(4, 2, Some(10)))],
      },
    ];
    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert!(midi.0.is_empty());

    player
      .tick(&state, &mut midi, now + Duration::from_millis(130))
      .unwrap();
    assert_eq!(player.state().row_index, 1);
    assert_eq!(player.state().time, Duration::from_millis(125));
    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 40, 64],
        vec![MIDI_PROGRAM_CHANGE, 1],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 38, 64],
        vec![MIDI_PROGRAM_CHANGE, 1],
        vec![MIDI_NOTE_ON, 37, 64],
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 39, 64],
      ]
    );
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(250))
    );
  }

  #[test]
  fn early_first_row_note_repeats_before_one_row_loop_boundaries() {
    let mut state = minimal_song();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(pitched_note(0, 1, Some(-90)))],
    });
    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert_eq!(midi.0.len(), 2);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(12_500))
    );
    player
      .tick(&state, &mut midi, now + Duration::from_micros(137_500))
      .unwrap();
    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, 1],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
      ]
    );
    assert_eq!(player.state().row_index, 0);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(250))
    );
  }

  #[test]
  fn repeated_pattern_occurrences_and_loops_each_play_once() {
    let mut state = minimal_song();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(pitched_note(0, 1, Some(30)))],
    });
    state.sequence.titles.push("Pattern".to_owned());
    state.sequence.end_of_song = 1;
    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(37_500))
    );

    player
      .tick(&state, &mut midi, now + Duration::from_micros(37_499))
      .unwrap();
    assert!(midi.0.is_empty());
    player
      .tick(&state, &mut midi, now + Duration::from_millis(540))
      .unwrap();
    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, 1],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
        vec![MIDI_NOTE_ON, 36, 64],
      ]
    );
    assert_eq!(player.state().sequence_index, 0);
    assert_eq!(player.state().time, Duration::ZERO);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(625))
    );
  }

  #[test]
  fn stopping_discards_pending_delayed_notes() {
    let mut state = minimal_song();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(pitched_note(0, 1, Some(90)))],
    });
    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    player.stop();

    assert!(player.next_deadline().is_none());
    assert!(
      player
        .tick(&state, &mut midi, now + Duration::from_secs(1))
        .is_err()
    );
    assert!(midi.0.is_empty());
  }

  #[test]
  fn past_early_note_from_next_loop_is_not_clamped_on_mid_song_start() {
    let mut state = minimal_song();
    state.patterns[0].rows_per_beat = NonZeroU8::new(1).unwrap();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(pitched_note(0, 1, Some(-90)))],
    });
    let mut short_pattern = state.patterns[0].clone();
    short_pattern.title = "Short".to_owned();
    short_pattern.num_rows = std::num::NonZero::new(20).unwrap();
    short_pattern.rows_per_beat = NonZeroU8::new(16).unwrap();
    short_pattern.tracks.clear();
    state.patterns.push(short_pattern);
    state.sequence.titles.push("Short".to_owned());
    state.sequence.end_of_song = 1;
    let now = Instant::now();
    let mut player = Player::default();
    player.state_mut().sequence_index = 1;
    player.state_mut().row_index = 19;
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert!(midi.0.is_empty());

    player
      .tick(&state, &mut midi, now + Duration::from_micros(706_249))
      .unwrap();
    assert!(midi.0.is_empty());
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(706_250))
    );
    player
      .tick(&state, &mut midi, now + Duration::from_micros(706_250))
      .unwrap();
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 1], vec![MIDI_NOTE_ON, 36, 64]]
    );
  }

  #[test]
  fn starting_mid_song_clamps_future_rows_without_replaying_earlier_rows() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(3).unwrap();
    state.patterns[0].tracks = vec![
      Track {
        notes: vec![
          Some(pitched_note(0, 1, None)),
          Some(pitched_note(1, 1, Some(-90))),
        ],
      },
      Track {
        notes: vec![
          None,
          Some(pitched_note(2, 2, Some(90))),
          Some(pitched_note(3, 2, Some(-90))),
        ],
      },
    ];
    let now = Instant::now();
    let mut player = Player::default();
    player.state_mut().row_index = 1;
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 1], vec![MIDI_NOTE_ON, 37, 64]]
    );
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(12_500))
    );
    player
      .tick(&state, &mut midi, now + Duration::from_micros(112_500))
      .unwrap();
    assert_eq!(
      &midi.0[2..],
      &[
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 39, 64],
        vec![MIDI_NOTE_ON, 38, 64],
      ]
    );
    assert_eq!(player.state().row_index, 1);
    player.stop();
    assert!(player.next_deadline().is_none());
  }

  #[test]
  fn early_note_can_cross_multiple_rows_and_a_pattern_boundary() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(20).unwrap();
    state.patterns[0].rows_per_beat = NonZeroU8::new(16).unwrap();
    let mut long_pattern = state.patterns[0].clone();
    long_pattern.title = "Long".to_owned();
    long_pattern.num_rows = std::num::NonZero::new(1).unwrap();
    long_pattern.rows_per_beat = NonZeroU8::new(1).unwrap();
    long_pattern.tracks.push(Track {
      notes: vec![Some(pitched_note(4, 1, Some(-90)))],
    });
    state.patterns.push(long_pattern);
    state.sequence.titles.push("Long".to_owned());
    state.sequence.end_of_song = 1;

    let now = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    player
      .tick(&state, &mut midi, now + Duration::from_micros(174_999))
      .unwrap();
    assert!(midi.0.is_empty());
    assert_eq!(player.state().row_index, 5);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(175))
    );
    player
      .tick(&state, &mut midi, now + Duration::from_millis(175))
      .unwrap();
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 1], vec![MIDI_NOTE_ON, 40, 64]]
    );
    assert_eq!(player.state().sequence_index, 0);
    assert_eq!(player.state().row_index, 5);
    player
      .tick(&state, &mut midi, now + Duration::from_millis(625))
      .unwrap();
    assert_eq!(player.state().sequence_index, 1);
    assert_eq!(midi.0.len(), 2);
    player
      .tick(&state, &mut midi, now + Duration::from_millis(1_300))
      .unwrap();
    assert_eq!(midi.0.len(), 3);
    assert_eq!(midi.0[2], vec![MIDI_NOTE_ON, 40, 64]);
  }

  #[test]
  fn starting_mid_song_clamps_a_later_pattern_note_that_is_already_due() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(20).unwrap();
    state.patterns[0].rows_per_beat = NonZeroU8::new(16).unwrap();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(pitched_note(0, 1, None))],
    });
    let mut long_pattern = state.patterns[0].clone();
    long_pattern.title = "Long".to_owned();
    long_pattern.num_rows = std::num::NonZero::new(1).unwrap();
    long_pattern.rows_per_beat = NonZeroU8::new(1).unwrap();
    long_pattern.tracks[0].notes = vec![Some(pitched_note(4, 2, Some(-90)))];
    state.patterns.push(long_pattern);
    state.sequence.titles.push("Long".to_owned());
    state.sequence.end_of_song = 1;

    let now = Instant::now();
    let mut player = Player::default();
    player.state_mut().row_index = 19;
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, now).unwrap();
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 2], vec![MIDI_NOTE_ON, 40, 64]]
    );
    assert_eq!(player.state().sequence_index, 0);
    assert_eq!(player.state().row_index, 19);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_micros(31_250))
    );
  }

  #[test]
  fn timeline_schedules_delays_after_all_row_tempo_effects() {
    let mut state = minimal_song();
    let effects = [
      Some(Effect::Delay(Delay::new(-90).unwrap())),
      None,
      Some(Effect::Delay(Delay::new(0).unwrap())),
      Some(Effect::Delay(Delay::new(90).unwrap())),
      Some(Effect::SetTempo(NonZeroU8::new(60).unwrap())),
      Some(Effect::SetTempo(NonZeroU8::new(240).unwrap())),
      Some(Effect::Delay(Delay::new(-30).unwrap())),
      Some(Effect::Delay(Delay::new(30).unwrap())),
    ];
    state.patterns[0].tracks = effects
      .into_iter()
      .map(|effect| Track {
        notes: vec![Some(test_note(effect))],
      })
      .collect();

    let timeline = Timeline::from_song(&state).unwrap();
    assert_eq!(timeline.rows[0][0].start_time, Duration::ZERO);
    assert_eq!(timeline.rows[0][0].duration, Duration::from_micros(62_500));
    assert_eq!(
      timeline.notes,
      [
        (-56_250_000, 0),
        (-18_750_000, 6),
        (0, 1),
        (0, 2),
        (0, 4),
        (0, 5),
        (18_750_000, 7),
        (56_250_000, 3),
      ]
      .map(|(time, track_index)| NoteEvent {
        time,
        sequence_index: 0,
        row_index: 0,
        track_index,
      })
    );
  }

  #[test]
  fn timeline_schedules_delays_in_single_percent_increments() {
    let mut state = minimal_song();
    state.patterns[0].tracks = [-99, -1, 0, 1, 99]
      .into_iter()
      .map(|delay| Track {
        notes: vec![Some(test_note(Some(Effect::Delay(
          Delay::new(delay).unwrap(),
        ))))],
      })
      .collect();

    let timeline = Timeline::from_song(&state).unwrap();
    assert_eq!(timeline.rows[0][0].duration, Duration::from_millis(125));
    assert_eq!(
      timeline.notes,
      [
        (-123_750_000, 0),
        (-1_250_000, 1),
        (0, 2),
        (1_250_000, 3),
        (123_750_000, 4),
      ]
      .map(|(time, track_index)| NoteEvent {
        time,
        sequence_index: 0,
        row_index: 0,
        track_index,
      })
    );
  }

  #[test]
  fn timeline_schedules_each_occurrence_and_sorts_notes_across_rows() {
    let mut state = minimal_song();
    state.patterns[0].num_rows = std::num::NonZero::new(3).unwrap();
    state.patterns[0].rows_per_beat = NonZeroU8::new(16).unwrap();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(test_note(None)), None, Some(test_note(None))],
    });

    let mut long_pattern = state.patterns[0].clone();
    long_pattern.title = "Long".to_owned();
    long_pattern.num_rows = std::num::NonZero::new(1).unwrap();
    long_pattern.rows_per_beat = NonZeroU8::new(1).unwrap();
    long_pattern.tracks[0].notes = vec![Some(test_note(Some(Effect::Delay(
      Delay::new(-90).unwrap(),
    ))))];
    state.patterns.push(long_pattern);
    state.sequence.titles.push("Long".to_owned());
    state.sequence.titles.push("Pattern".to_owned());
    state
      .sequence
      .titles
      .push("Missing pool pattern".to_owned());
    state.sequence.end_of_song = 2;

    let timeline = Timeline::from_song(&state).unwrap();
    assert_eq!(
      timeline.rows[1][0].start_time,
      Duration::from_micros(93_750)
    );
    assert_eq!(timeline.rows[1][0].duration, Duration::from_millis(500));
    assert_eq!(
      timeline.rows[2][0].start_time,
      Duration::from_micros(593_750)
    );
    assert_eq!(
      timeline.notes,
      [
        (-356_250_000, 1, 0),
        (0, 0, 0),
        (62_500_000, 0, 2),
        (593_750_000, 2, 0),
        (656_250_000, 2, 2),
      ]
      .map(|(time, sequence_index, row_index)| NoteEvent {
        time,
        sequence_index,
        row_index,
        track_index: 0,
      })
    );
  }

  #[test]
  fn starts_with_program_changes_and_notes_in_track_order() {
    let state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    let mut player_state = PlayerState {
      sequence_index: 1,
      ..Default::default()
    };
    let mut midi = RecordingMidiSink(Vec::new());

    Playback::start(&state, &mut player_state, &mut midi, Instant::now()).unwrap();

    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 36, 25],
        vec![MIDI_PROGRAM_CHANGE, 4],
        vec![MIDI_NOTE_ON, 40, 102]
      ]
    );
  }

  #[test]
  fn plays_missing_notes_in_declared_rows_as_empty() {
    let mut serialized_state: serde_json::Value =
      serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    serialized_state["patterns"][0]["tracks"][0]["notes"] = serde_json::json!([null]);
    let state: AppState = serde_json::from_value(serialized_state).unwrap();
    let mut player_state = PlayerState {
      row_index: 7,
      ..Default::default()
    };
    let mut midi = RecordingMidiSink(Vec::new());

    Playback::start(&state, &mut player_state, &mut midi, Instant::now()).unwrap();

    assert!(midi.0.is_empty());
  }

  #[test]
  fn fails_to_start_when_a_song_pattern_is_missing() {
    let mut state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    state.sequence.titles[1] = "Missing pattern".to_owned();
    let mut player_state = PlayerState {
      sequence_index: 1,
      row_index: 7,
      ..Default::default()
    };
    let mut midi = RecordingMidiSink(Vec::new());

    assert!(matches!(
      Playback::start(&state, &mut player_state, &mut midi, Instant::now()),
      Err(error) if error.to_string() == "Pattern not found: Missing pattern"
    ));
  }

  #[test]
  fn player_start_and_stop_manage_the_active_playback() {
    let state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut player = Player::default();
    player.state_mut().sequence_index = 1;

    assert!(player.next_deadline().is_none());
    player.start(&state, &mut midi, Instant::now()).unwrap();
    assert!(player.next_deadline().is_some());
    let messages_before_stop = midi.0.clone();
    player.stop();

    assert!(player.next_deadline().is_none());
    assert_eq!(midi.0, messages_before_stop);
    player.stop();
    assert!(player.next_deadline().is_none());
  }

  #[test]
  fn minimal_one_track_one_note_song_plays_loops_and_stops() {
    let mut state = minimal_song();
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(Note {
        octave: Octave(3),
        semitone: Semitone(4),
        instrument: InstrumentIndex(2),
        velocity: Velocity(80),
        effect: None,
      })],
    });
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut player = Player::default();

    player.start(&state, &mut midi, now).unwrap();

    assert_eq!(player.state().sequence_index, 0);
    assert_eq!(player.state().row_index, 0);
    assert_eq!(player.state().time, Duration::ZERO);
    assert_eq!(
      player.next_deadline(),
      Some(now + Duration::from_millis(125))
    );
    let initial_messages = midi.0.clone();

    player
      .tick(&state, &mut midi, now + Duration::from_millis(125))
      .unwrap();

    assert_eq!(player.state().sequence_index, 0);
    assert_eq!(player.state().row_index, 0);
    assert_eq!(initial_messages.len(), 2);
    assert_eq!(midi.0.len(), 3);
    assert_eq!(midi.0[2], initial_messages[1]);

    player.stop();
    assert!(player.next_deadline().is_none());
  }

  #[test]
  fn emits_a_program_change_and_note_on_without_a_note_off() {
    let mut midi = RecordingMidiSink(Vec::new());
    let player = Player::default();
    let note = Note {
      octave: Octave(3),
      semitone: Semitone(4),
      instrument: InstrumentIndex(2),
      velocity: Velocity(80),
      effect: None,
    };

    player.emit_note_on(&note, &mut midi).unwrap();

    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 2], vec![MIDI_NOTE_ON, 40, 102]]
    );
  }

  #[test]
  fn linearizes_simultaneous_notes_with_program_changes() {
    let state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    let mut player_state = PlayerState {
      sequence_index: 1,
      ..Default::default()
    };
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&state, &mut player_state, &mut midi, now).unwrap();

    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, 2],
        vec![MIDI_NOTE_ON, 36, 25],
        vec![MIDI_PROGRAM_CHANGE, 4],
        vec![MIDI_NOTE_ON, 40, 102]
      ]
    );

    let next_deadline = playback.next_deadline();
    playback
      .tick(&state, &mut player_state, &mut midi, next_deadline)
      .unwrap();
    assert_eq!(midi.0.len(), 4);
  }

  #[test]
  fn derives_row_time_and_schedule_from_left_to_right_tempo_effects() {
    let state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    let now = Instant::now();
    let mut player_state = PlayerState {
      sequence_index: 1,
      row_index: 1,
      ..Default::default()
    };
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&state, &mut player_state, &mut midi, now).unwrap();

    assert_eq!(player_state.time, Duration::from_micros(1_031_250));
    assert_eq!(
      playback.next_deadline(),
      now + Duration::from_micros(31_250)
    );

    playback
      .tick(
        &state,
        &mut player_state,
        &mut midi,
        now + Duration::from_millis(250),
      )
      .unwrap();

    assert_eq!(player_state.time, Duration::from_micros(1_281_250));
    assert_eq!(
      playback.next_deadline(),
      now + Duration::from_micros(281_250)
    );
  }

  #[test]
  fn edited_tempo_is_used_for_row_timing_and_survives_serialization() {
    let mut state = crate::state::test_state();
    state.initial_bpm = NonZeroU8::new(80).unwrap();
    state.set_focused_note_effect(Some(Effect::SetTempo(NonZeroU8::new(100).unwrap())));
    let now = Instant::now();
    state.set_focused_note_tempo_digit(2, now);
    state.set_focused_note_tempo_digit(0, now);

    let json = serde_json::to_string(&state).unwrap();
    let loaded: AppState = serde_json::from_str(&json).unwrap();
    assert_eq!(
      loaded.get_current_pattern().get_note(0, 0).unwrap().effect,
      Some(Effect::SetTempo(NonZeroU8::new(120).unwrap()))
    );
    let timeline = Timeline::from_song(&loaded).unwrap();
    assert_eq!(timeline.rows[0][0].duration, Duration::from_millis(125));

    state.focus.track_column = crate::state::pattern::TrackColumn::Effect;
    state.adjust_focused_note(true, true);
    let timeline = Timeline::from_song(&state).unwrap();
    assert_eq!(
      timeline.rows[0][0].duration,
      row_duration(
        NonZeroU8::new(121).unwrap(),
        state.patterns[0].rows_per_beat
      )
    );
  }

  #[test]
  fn advances_to_the_next_pattern_with_its_tempo_and_resolution() {
    let state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    let mut player_state = PlayerState {
      sequence_index: 1,
      row_index: 15,
      ..Default::default()
    };
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&state, &mut player_state, &mut midi, now).unwrap();

    assert_eq!(player_state.time, Duration::from_micros(1_468_750));
    playback
      .tick(
        &state,
        &mut player_state,
        &mut midi,
        now + Duration::from_micros(31_250),
      )
      .unwrap();

    assert_eq!(player_state.sequence_index, 2);
    assert_eq!(player_state.row_index, 0);
    assert_eq!(player_state.time, Duration::from_millis(1_500));
    assert_eq!(
      playback.next_deadline(),
      now + Duration::from_micros(156_250)
    );
    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 2], vec![MIDI_NOTE_ON, 35, 115]]
    );
  }

  #[test]
  fn loops_after_the_last_song_pattern() {
    let mut state: AppState = serde_json::from_str(include_str!("../tests/test.json")).unwrap();
    state.sequence.end_of_song = 0;
    let mut player_state = PlayerState {
      row_index: 7,
      ..Default::default()
    };
    let now = Instant::now();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut playback = Playback::start(&state, &mut player_state, &mut midi, now).unwrap();

    playback
      .tick(
        &state,
        &mut player_state,
        &mut midi,
        now + Duration::from_millis(125),
      )
      .unwrap();

    assert_eq!(player_state.sequence_index, 0);
    assert_eq!(player_state.row_index, 0);
  }
}
