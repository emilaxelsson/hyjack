pub mod editor_settings;
pub mod instruments;
pub mod name;
pub mod pattern;
pub mod sequence;

use std::num::NonZeroU8;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::state::editor_settings::EditorSettings;
use crate::state::instruments::{InstrumentIndex, Instruments};
use crate::state::name::normalize_and_validate_name;
use crate::state::pattern::{
  Focus, Note, Octave, Pattern, PatternBounds, PatternDefaults, Semitone, Track, TrackColumn,
  Velocity,
};
use crate::state::sequence::Sequence;
use crate::utils::{Adjustable, fresh_name, nonzero_len};

#[derive(Serialize, Deserialize)]
pub struct AppState {
  pub hyjack_version: String,
  pub song_title: String,
  pub initial_bpm: NonZeroU8,
  pub pattern_defaults: PatternDefaults,
  pub track_titles: NonEmpty<String>,
  pub patterns: NonEmpty<Pattern>,
  pub sequence: Sequence,
  pub instruments: Instruments,
  pub focus: Focus,
  #[serde(skip, default)]
  pub editor_settings: EditorSettings,
  #[serde(skip, default)]
  instrument_digit_entry: Option<DigitEntry>,
  #[serde(skip, default)]
  velocity_digit_entry: Option<DigitEntry>,
}

#[derive(Clone, Copy)]
struct DigitEntry {
  digit: u8,
  entered_at: Instant,
}

fn enter_digit(entry: &mut Option<DigitEntry>, digit: u8, now: Instant) -> u8 {
  const DIGIT_PAIR_TIMEOUT: Duration = Duration::from_secs(1);

  let previous_digit = entry
    .filter(|previous| now.duration_since(previous.entered_at) <= DIGIT_PAIR_TIMEOUT)
    .map(|previous| previous.digit);

  *entry = Some(DigitEntry {
    digit,
    entered_at: now,
  });

  previous_digit.map_or(digit, |previous| previous * 10 + digit)
}

impl AppState {
  pub fn load_from_file(path: &Path) -> Result<Self> {
    let json = std::fs::read_to_string(path)?;
    let mut state: Self = serde_json::from_str(&json)?;
    for pattern in &state.patterns {
      anyhow::ensure!(
        pattern.tracks.len() <= state.track_titles.len(),
        "Pattern '{}' has {} tracks but only {} track titles are configured",
        pattern.title,
        pattern.tracks.len(),
        state.track_titles.len()
      );
      for track in &pattern.tracks {
        anyhow::ensure!(
          track.notes.len() <= pattern.num_rows.get(),
          "Pattern '{}' has a track with {} notes but only {} rows",
          pattern.title,
          track.notes.len(),
          pattern.num_rows
        );
      }
    }
    state.hyjack_version = env!("CARGO_PKG_VERSION").to_owned();
    Ok(state)
  }

  pub fn save_to_file(&mut self, path: &Path) -> Result<()> {
    self.prune_empty_tracks();
    self.prune_empty_notes();
    let json = serde_json::to_string_pretty(self)?;
    std::fs::write(path, json)?;
    Ok(())
  }

  pub fn prune_empty_tracks(&mut self) {
    for pattern in self.patterns.iter_mut() {
      while pattern
        .tracks
        .last()
        .is_some_and(|track| track.notes.iter().all(Option::is_none))
      {
        pattern.tracks.pop();
      }
    }
  }

  pub fn prune_empty_notes(&mut self) {
    for pattern in self.patterns.iter_mut() {
      for track in &mut pattern.tracks {
        while track.notes.last().is_some_and(Option::is_none) {
          track.notes.pop();
        }
      }
    }
  }

  pub fn rename_song(&mut self, new_title: String) -> Result<()> {
    let normalized = normalize_and_validate_name("Song title", &new_title)?;
    self.song_title = normalized;
    Ok(())
  }

  // Get the index of the currently focused pattern in the `patterns` list.
  //
  // This has linear time complexity. Maybe switch to hash map if this becomes a
  // performance issue.
  pub fn get_current_pattern_index(&self) -> usize {
    let title = self
      .sequence
      .titles
      .get(self.sequence.focus_index)
      .expect("Sequence focus index is out of bounds");

    self
      .patterns
      .iter()
      .position(|p| p.title == *title)
      .unwrap_or_else(|| panic!("Pattern not found: {}", title))
  }

  pub fn get_current_pattern(&self) -> &Pattern {
    self.patterns.get(self.get_current_pattern_index()).unwrap()
  }

  fn current_pattern_bounds(&self) -> PatternBounds {
    PatternBounds {
      num_rows: self.get_current_pattern().num_rows,
      num_tracks: nonzero_len(&self.track_titles),
    }
  }

  /// Ensure that the pattern focus is within the bounds of the current pattern.
  pub fn refocus_pattern(&mut self) {
    self.focus.refocus(&self.current_pattern_bounds());
  }

  /// Navigate to the previous pattern row, crossing to the preceding sequence entry at row zero.
  pub fn to_previous_pattern_row(&mut self) {
    if self.focus.row_index > 0 {
      self.focus.to_previous_row();
      return;
    }

    let previous_sequence_index = self.sequence.focus_index.saturating_sub(1);
    if previous_sequence_index == self.sequence.focus_index {
      return;
    }

    self.sequence.focus_index = previous_sequence_index;
    let bounds = self.current_pattern_bounds();
    self.focus.row_index = bounds.num_rows.get() - 1;
    self.focus.refocus(&bounds);
  }

  /// Navigate to the next pattern row, crossing to the following sequence entry at the last row.
  pub fn to_next_pattern_row(&mut self) {
    let bounds = self.current_pattern_bounds();
    if self.focus.row_index + 1 < bounds.num_rows.get() {
      self.focus.to_next_row(bounds);
      return;
    }

    if self.sequence.focus_index + 1 >= self.sequence.titles.len() {
      return;
    }

    self.sequence.focus_index += 1;
    self.focus.row_index = 0;
    self.refocus_pattern();
  }

  /// Navigate to the previous pattern in the sequence.
  pub fn to_previous_pattern(&mut self) {
    let previous_sequence_index = self.sequence.focus_index.saturating_sub(1);
    if previous_sequence_index == self.sequence.focus_index {
      return;
    }

    self.sequence.focus_index = previous_sequence_index;
    self.refocus_pattern();
  }

  /// Navigate to the next pattern in the sequence.
  pub fn to_next_pattern(&mut self) {
    if self.sequence.focus_index + 1 >= self.sequence.titles.len() {
      return;
    }

    self.sequence.focus_index += 1;
    self.refocus_pattern();
  }

  /// Navigate to the first pattern column in the current row.
  pub fn to_first_pattern_column(&mut self) {
    self.focus.track_index = 0;
    self.focus.track_column = TrackColumn::Note;
  }

  /// Navigate to the last pattern column in the current row.
  pub fn to_last_pattern_column(&mut self) {
    self.focus.track_index = self.current_pattern_bounds().num_tracks.get() - 1;
    self.focus.track_column = TrackColumn::Effect;
  }

  /// Navigate to the previous track while retaining the focused column.
  pub fn to_previous_pattern_track(&mut self) {
    self.focus.to_previous_track();
  }

  /// Navigate to the next track while retaining the focused column.
  pub fn to_next_pattern_track(&mut self) {
    self.focus.to_next_track(self.current_pattern_bounds());
  }

  /// Navigate to the previous editable field in the current pattern row.
  pub fn to_previous_pattern_field(&mut self) {
    self.focus.to_previous_track_field();
  }

  /// Navigate to the next editable field in the current pattern row.
  pub fn to_next_pattern_field(&mut self) {
    self
      .focus
      .to_next_track_field(self.current_pattern_bounds());
  }

  /// Navigate to the first row of the current pattern.
  pub fn to_first_pattern_row(&mut self) {
    self.focus.row_index = 0;
  }

  /// Navigate to the last row of the current pattern.
  pub fn to_last_pattern_row(&mut self) {
    self.focus.row_index = self.current_pattern_bounds().num_rows.get() - 1;
  }

  /// Navigate to the first row of the song.
  pub fn to_first_song_row(&mut self) {
    self.sequence.focus_index = 0;
    self.focus.row_index = 0;
    self.refocus_pattern();
  }

  /// Navigate to the final song row, or to the end of the pool when already there.
  pub fn to_last_song_or_pool_row(&mut self) {
    self.sequence.focus_index = if self.sequence.focus_index >= self.sequence.end_of_song {
      self.sequence.titles.len() - 1
    } else {
      self.sequence.end_of_song
    };
    self.focus.row_index = self.current_pattern_bounds().num_rows.get() - 1;
    self.refocus_pattern();
  }

  /// Navigate to the first row, or the previous pattern when already on the first row.
  pub fn page_up_pattern(&mut self) {
    if self.focus.row_index == 0 {
      self.to_previous_pattern();
    } else {
      self.to_first_pattern_row();
    }
  }

  /// Navigate to the last row, or the next pattern when already on the last row.
  pub fn page_down_pattern(&mut self) {
    if self.focus.row_index + 1 == self.current_pattern_bounds().num_rows.get() {
      self.to_next_pattern();
    } else {
      self.to_last_pattern_row();
    }
  }

  pub fn rename_focused_pattern(&mut self, new_title: String) -> Result<()> {
    let normalized = normalize_and_validate_name("Pattern title", &new_title)?;
    let old_title = self.get_current_pattern().title.clone();
    let current_index = self.get_current_pattern_index();

    let pattern_title_exists = {
      self
        .patterns
        .iter()
        .enumerate()
        .any(|(index, pattern)| index != current_index && pattern.title == normalized)
    };

    if pattern_title_exists {
      anyhow::bail!("Pattern title already exists");
    }

    let pattern = self
      .patterns
      .get_mut(self.get_current_pattern_index())
      .unwrap();
    pattern.title = normalized.clone();

    self.sequence.titles.iter_mut().for_each(|title| {
      if *title == old_title {
        *title = normalized.clone();
      }
    });

    Ok(())
  }

  /// Duplicate the focused pattern and insert its title below the current sequence entry.
  pub fn duplicate_focused_pattern(&mut self) {
    let mut pattern = self.get_current_pattern().clone();
    pattern.title = fresh_name(
      &pattern.title,
      self.patterns.iter().map(|existing| existing.title.as_str()),
    );

    let focused_last_song_title = self.sequence.focus_index == self.sequence.end_of_song;
    self.sequence.insert_title_at_focus(pattern.title.clone());
    if focused_last_song_title {
      self.sequence.end_of_song += 1;
    }
    self.sequence.focus_index += 1;
    self.patterns.push(pattern);
  }

  /// Create a new pattern from the song defaults and insert it below the focused sequence entry.
  pub fn insert_new_pattern(&mut self) {
    let existing_titles: Vec<_> = self
      .patterns
      .iter()
      .map(|pattern| pattern.title.as_str())
      .collect();
    let title = (0..)
      .map(|number| match number {
        0 => "New pattern".to_owned(),
        number => format!("New pattern {number}"),
      })
      .find(|title| !existing_titles.contains(&title.as_str()))
      .unwrap();
    let pattern = self.pattern_defaults.make_pattern(title.clone());

    let focused_last_song_title = self.sequence.focus_index == self.sequence.end_of_song;
    self.sequence.insert_title_at_focus(title);
    if focused_last_song_title {
      self.sequence.end_of_song += 1;
    }
    self.sequence.focus_index += 1;
    self.patterns.push(pattern);
    self.refocus_pattern();
  }

  /// Set the note at the current pattern focus.
  pub fn set_focused_note(&mut self, note: Note) {
    if self.focus.track_index >= self.track_titles.len() {
      return;
    }
    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    while pattern.tracks.len() <= self.focus.track_index {
      pattern.tracks.push(Track { notes: vec![] });
    }
    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
  }

  /// Copy the focused note's octave, velocity, and instrument to the editor settings.
  pub fn copy_focused_note_attributes_to_editor_settings(&mut self) {
    let Some(note) = self
      .get_current_pattern()
      .get_note(self.focus.row_index, self.focus.track_index)
    else {
      return;
    };

    self.editor_settings.octave = note.octave;
    self.editor_settings.velocity = note.velocity;
    self.editor_settings.instrument = note.instrument;
  }

  /// Remove the note at the current pattern focus.
  pub fn remove_focused_note(&mut self) {
    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    pattern.remove_note(self.focus.row_index, self.focus.track_index);
  }

  /// Adjust the value in the focused pattern column.
  pub fn adjust_focused_note(&mut self, increase: bool, small_step: bool) {
    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    let Some(mut note) = pattern.get_note(self.focus.row_index, self.focus.track_index) else {
      return;
    };

    match self.focus.track_column {
      TrackColumn::Note => {
        note.adjust((), increase, small_step);
      }
      TrackColumn::Instrument => {
        let max_instrument = self.instruments.max_index().unwrap_or(InstrumentIndex(0));
        note.instrument.adjust(max_instrument, increase, small_step);
      }
      TrackColumn::Velocity => {
        note.velocity.adjust((), increase, small_step);
      }
      TrackColumn::Effect => {}
    }

    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
  }

  /// Set the value in the focused pattern column to its minimum or maximum.
  pub fn set_focused_note_to_bound(&mut self, maximum: bool) {
    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    let Some(mut note) = pattern.get_note(self.focus.row_index, self.focus.track_index) else {
      return;
    };

    match self.focus.track_column {
      TrackColumn::Note => {
        note.octave = Octave(if maximum { Note::MAX_OCTAVE } else { 0 });
        note.semitone = Semitone(if maximum { Note::MAX_SEMITONE } else { 0 });
      }
      TrackColumn::Instrument => {
        note.instrument = if maximum {
          self.instruments.max_index().unwrap_or(InstrumentIndex(0))
        } else {
          InstrumentIndex(0)
        };
      }
      TrackColumn::Velocity => {
        note.velocity = Velocity(if maximum { Velocity::MAX } else { 0 });
      }
      TrackColumn::Effect => {}
    }

    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
  }

  /// Set the instrument of the focused note from a digit typed in the pattern editor.
  pub fn set_focused_note_instrument_digit(&mut self, digit: u8, now: Instant) {
    let instrument = enter_digit(&mut self.instrument_digit_entry, digit, now);

    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    let Some(mut note) = pattern.get_note(self.focus.row_index, self.focus.track_index) else {
      return;
    };

    note.instrument = InstrumentIndex(instrument);
    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
  }

  /// Set the velocity of the focused note from a digit typed in the pattern editor.
  pub fn set_focused_note_velocity_digit(&mut self, digit: u8, now: Instant) {
    let velocity = enter_digit(&mut self.velocity_digit_entry, digit, now);

    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    let Some(mut note) = pattern.get_note(self.focus.row_index, self.focus.track_index) else {
      return;
    };

    note.velocity = Velocity(velocity);
    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
  }

  pub fn step_rows(&mut self) {
    let step = self.editor_settings.step;
    self.focus.jump_rows(step, self.current_pattern_bounds());
  }
}

#[cfg(test)]
pub fn test_state() -> AppState {
  use std::num::NonZero;

  AppState {
    hyjack_version: "test".to_owned(),
    song_title: "Test song".to_owned(),
    initial_bpm: NonZeroU8::new(120).unwrap(),
    pattern_defaults: PatternDefaults::default(),
    track_titles: NonEmpty::new("Track 1".to_owned()),
    patterns: NonEmpty::new(Pattern {
      title: "Pattern".to_owned(),
      tracks: vec![Track {
        notes: vec![Some(Note {
          octave: Octave(3),
          semitone: Semitone(0),
          instrument: InstrumentIndex(0),
          velocity: Velocity(50),
          effect: None,
        })],
      }],
      num_rows: NonZero::new(1).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    }),
    sequence: Sequence {
      titles: NonEmpty::new("Pattern".to_owned()),
      focus_index: 0,
      end_of_song: 0,
    },
    instruments: Instruments::DirectMapped(vec![]),
    focus: Focus {
      row_index: 0,
      track_index: 0,
      track_column: TrackColumn::Instrument,
    },
    editor_settings: EditorSettings::default(),
    instrument_digit_entry: None,
    velocity_digit_entry: None,
  }
}

#[cfg(test)]
pub fn minimal_song() -> AppState {
  use std::num::NonZero;

  let mut state = test_state();

  state.track_titles = NonEmpty::new("Track".to_owned());
  state.patterns = NonEmpty::new(Pattern {
    title: "Pattern".to_owned(),
    tracks: vec![],
    num_rows: NonZero::new(1).unwrap(),
    rows_per_beat: NonZeroU8::new(4).unwrap(),
  });
  state.sequence = Sequence {
    titles: NonEmpty::new("Pattern".to_owned()),
    focus_index: 0,
    end_of_song: 0,
  };

  state
}

#[cfg(test)]
mod tests {
  use std::num::{NonZero, NonZeroU8};

  use super::*;
  use crate::state::instruments::{InstrumentIndex, InstrumentMetadata, InstrumentName};
  use crate::state::pattern::{Octave, Semitone, Track};

  #[test]
  fn prune_empty_tracks_removes_only_trailing_empty_tracks_from_each_pattern() {
    let mut state = test_state();
    state.patterns[0].tracks.push(Track { notes: vec![None] });
    state.patterns[0].tracks.push(Track { notes: vec![] });

    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: vec![
        Track { notes: vec![] },
        state.patterns[0].tracks[0].clone(),
        Track { notes: vec![None] },
        Track { notes: vec![] },
      ],
      num_rows: NonZero::new(1).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });
    state.patterns.push(Pattern {
      title: "Empty pattern".to_owned(),
      tracks: vec![Track { notes: vec![] }, Track { notes: vec![None] }],
      num_rows: NonZero::new(1).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });

    state.prune_empty_tracks();

    assert_eq!(state.patterns[0].tracks.len(), 1);
    assert_eq!(state.patterns[1].tracks.len(), 2);
    assert!(state.patterns[1].tracks[0].notes.is_empty());
    assert!(state.patterns[1].tracks[1].notes[0].is_some());
    assert!(state.patterns[2].tracks.is_empty());
  }

  #[test]
  fn prune_empty_notes_removes_trailing_none_notes_throughout_song() {
    let mut state = test_state();
    state.patterns[0].tracks[0].notes.extend([None, None]);

    let note = state.patterns[0].tracks[0].notes[0].clone();
    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: vec![
        Track {
          notes: vec![note, None, None],
        },
        Track {
          notes: vec![None, None],
        },
        Track { notes: vec![] },
      ],
      num_rows: NonZero::new(3).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });

    state.prune_empty_notes();

    assert_eq!(state.patterns[0].tracks[0].notes.len(), 1);
    assert!(state.patterns[0].tracks[0].notes[0].is_some());
    assert_eq!(state.patterns[1].tracks[0].notes.len(), 1);
    assert!(state.patterns[1].tracks[0].notes[0].is_some());
    assert!(state.patterns[1].tracks[1].notes.is_empty());
    assert!(state.patterns[1].tracks[2].notes.is_empty());
  }

  #[test]
  fn loading_song_updates_version() {
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "hyjack-load-version-{}-{timestamp}.json",
      std::process::id()
    ));
    let mut serialized_state = serde_json::to_value(test_state()).unwrap();
    serialized_state["hyjack_version"] = "old-version".into();
    std::fs::write(&path, serde_json::to_vec(&serialized_state).unwrap()).unwrap();

    let state = AppState::load_from_file(&path).unwrap();
    let file_state: serde_json::Value =
      serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    std::fs::remove_file(path).unwrap();

    assert_eq!(state.hyjack_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(file_state["hyjack_version"], "old-version");
  }

  #[test]
  fn loading_song_rejects_more_tracks_than_track_titles_but_allows_fewer() {
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "hyjack-load-track-titles-{}-{timestamp}.json",
      std::process::id()
    ));
    let mut serialized_state = serde_json::to_value(test_state()).unwrap();
    serialized_state["track_titles"] = serde_json::json!(["Track 1", "Track 2"]);
    std::fs::write(&path, serde_json::to_vec(&serialized_state).unwrap()).unwrap();

    let state = AppState::load_from_file(&path).unwrap();
    assert_eq!(state.get_current_pattern().tracks.len(), 1);

    let first_track = serialized_state["patterns"][0]["tracks"][0].clone();
    serialized_state["patterns"][0]["tracks"]
      .as_array_mut()
      .unwrap()
      .push(first_track);
    let second_track = serialized_state["patterns"][0]["tracks"][0].clone();
    serialized_state["patterns"][0]["tracks"]
      .as_array_mut()
      .unwrap()
      .push(second_track);
    std::fs::write(&path, serde_json::to_vec(&serialized_state).unwrap()).unwrap();

    let error = AppState::load_from_file(&path).err().unwrap();
    std::fs::remove_file(path).unwrap();

    assert!(
      error
        .to_string()
        .contains("only 2 track titles are configured")
    );
  }

  #[test]
  fn loading_song_rejects_more_notes_than_pattern_rows_but_allows_fewer() {
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "hyjack-load-track-notes-{}-{timestamp}.json",
      std::process::id()
    ));
    let mut serialized_state = serde_json::to_value(test_state()).unwrap();
    serialized_state["patterns"][0]["num_rows"] = 2.into();
    std::fs::write(&path, serde_json::to_vec(&serialized_state).unwrap()).unwrap();

    let state = AppState::load_from_file(&path).unwrap();
    assert_eq!(state.get_current_pattern().num_rows.get(), 2);
    assert_eq!(state.get_current_pattern().tracks[0].notes.len(), 1);

    let note = serialized_state["patterns"][0]["tracks"][0]["notes"][0].clone();
    serialized_state["patterns"][0]["tracks"][0]["notes"]
      .as_array_mut()
      .unwrap()
      .extend([note.clone(), note]);
    std::fs::write(&path, serde_json::to_vec(&serialized_state).unwrap()).unwrap();

    let error = AppState::load_from_file(&path).err().unwrap();
    std::fs::remove_file(path).unwrap();

    assert!(error.to_string().contains("3 notes but only 2 rows"));
  }

  fn focused_instrument(state: &AppState) -> InstrumentIndex {
    state
      .get_current_pattern()
      .get_note(state.focus.row_index, state.focus.track_index)
      .unwrap()
      .instrument
  }

  fn focused_velocity(state: &AppState) -> Velocity {
    state
      .get_current_pattern()
      .get_note(state.focus.row_index, state.focus.track_index)
      .unwrap()
      .velocity
  }

  fn focused_octave(state: &AppState) -> Octave {
    state
      .get_current_pattern()
      .get_note(state.focus.row_index, state.focus.track_index)
      .unwrap()
      .octave
  }

  fn focused_semitone(state: &AppState) -> Semitone {
    state
      .get_current_pattern()
      .get_note(state.focus.row_index, state.focus.track_index)
      .unwrap()
      .semitone
  }

  #[test]
  fn duplicate_focused_pattern_inserts_a_unique_copy_below_the_current_title() {
    let mut state = test_state();
    state.sequence.titles.push("Pattern".to_owned());
    state.sequence.end_of_song = 1;
    state.sequence.focus_index = 0;
    state.patterns.push(Pattern {
      title: "Pattern copy".to_owned(),
      tracks: vec![Track { notes: vec![] }],
      num_rows: NonZero::new(1).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });

    state.duplicate_focused_pattern();

    assert_eq!(
      state.sequence.titles,
      NonEmpty::from_vec(vec![
        "Pattern".to_owned(),
        "Pattern copy1".to_owned(),
        "Pattern".to_owned(),
      ])
      .unwrap()
    );
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.sequence.end_of_song, 2);
    assert_eq!(state.patterns.len(), 3);
    assert_eq!(state.get_current_pattern().title, "Pattern copy1");
    assert_eq!(state.track_titles[0], "Track 1");

    state.duplicate_focused_pattern();

    assert_eq!(state.get_current_pattern().title, "Pattern copy2");
  }

  #[test]
  fn duplicate_focused_pattern_keeps_a_copy_of_the_last_song_title_out_of_the_pool() {
    let mut state = test_state();
    state.sequence.titles.push("Pattern".to_owned());
    state.sequence.titles.push("Pattern".to_owned());
    state.sequence.end_of_song = 1;
    state.sequence.focus_index = 1;

    state.duplicate_focused_pattern();

    assert_eq!(
      state.sequence.titles,
      NonEmpty::from_vec(vec![
        "Pattern".to_owned(),
        "Pattern".to_owned(),
        "Pattern copy".to_owned(),
        "Pattern".to_owned(),
      ])
      .unwrap()
    );
    assert_eq!(state.sequence.focus_index, 2);
    assert_eq!(state.sequence.end_of_song, 2);
    assert_eq!(state.sequence.pool_size(), 1);
  }

  #[test]
  fn insert_new_pattern_uses_defaults_and_selects_the_new_song_entry() {
    let mut state = test_state();
    state.pattern_defaults = PatternDefaults {
      num_rows: NonZero::new(8).unwrap(),
      rows_per_beat: NonZeroU8::new(2).unwrap(),
    };
    state.track_titles = NonEmpty::from_vec(vec!["Drums".to_owned(), "Bass".to_owned()]).unwrap();

    state.insert_new_pattern();

    assert_eq!(
      state.sequence.titles,
      NonEmpty::from_vec(vec!["Pattern".to_owned(), "New pattern".to_owned()]).unwrap()
    );
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.sequence.end_of_song, 1);
    assert_eq!(state.focus.row_index, 0);
    assert_eq!(state.focus.track_index, 0);
    let pattern = state.get_current_pattern();
    assert_eq!(pattern.num_rows.get(), 8);
    assert_eq!(pattern.rows_per_beat.get(), 2);
    assert!(pattern.tracks.is_empty());
    assert_eq!(state.track_titles[0], "Drums");
    assert_eq!(state.track_titles[1], "Bass");

    state.insert_new_pattern();
    assert_eq!(state.get_current_pattern().title, "New pattern 1");

    state.insert_new_pattern();
    assert_eq!(state.get_current_pattern().title, "New pattern 2");
    assert_eq!(
      state.sequence.titles,
      NonEmpty::from_vec(vec![
        "Pattern".to_owned(),
        "New pattern".to_owned(),
        "New pattern 1".to_owned(),
        "New pattern 2".to_owned(),
      ])
      .unwrap()
    );
  }

  #[test]
  fn new_patterns_use_song_defaults() {
    let mut state = test_state();
    state.pattern_defaults = PatternDefaults {
      num_rows: NonZero::new(8).unwrap(),
      rows_per_beat: NonZeroU8::new(2).unwrap(),
    };
    state.track_titles = NonEmpty::from_vec(vec!["Drums".to_owned(), "Bass".to_owned()]).unwrap();

    let pattern = state
      .pattern_defaults
      .make_pattern("New pattern".to_owned());

    assert_eq!(pattern.title, "New pattern");
    assert_eq!(pattern.num_rows.get(), 8);
    assert_eq!(pattern.rows_per_beat.get(), 2);
    assert!(pattern.tracks.is_empty());
    assert_eq!(state.track_titles[0], "Drums");
    assert_eq!(state.track_titles[1], "Bass");
  }

  #[test]
  fn inserting_note_materializes_only_required_implicit_tracks_and_rows() {
    let mut state = test_state();
    state.track_titles = NonEmpty::from_vec(vec!["Drums".to_owned(), "Bass".to_owned()]).unwrap();
    state.patterns.get_mut(0).unwrap().num_rows = NonZero::new(3).unwrap();
    state.patterns.get_mut(0).unwrap().tracks.clear();
    state.focus.track_index = 1;
    state.focus.row_index = 2;
    let note = state.editor_settings.make_note(Semitone(0));

    state.set_focused_note(note);

    let pattern = state.get_current_pattern();
    assert_eq!(pattern.tracks.len(), 2);
    assert!(pattern.tracks[0].notes.is_empty());
    assert_eq!(pattern.tracks[1].notes.len(), 3);
    assert_eq!(pattern.get_note(2, 1).unwrap().semitone, Semitone(0));
  }

  #[test]
  fn pattern_row_navigation_crosses_sequence_entries_at_pattern_boundaries() {
    let mut state = test_state();
    state.patterns.get_mut(0).unwrap().num_rows = NonZero::new(3).unwrap();
    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: vec![Track { notes: vec![] }],
      num_rows: NonZero::new(2).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });
    state.sequence.titles.push("Second pattern".to_owned());

    state.to_next_pattern_row();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 1);

    state.focus.row_index = 2;
    state.to_next_pattern_row();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 0);

    state.to_previous_pattern_row();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 2);

    state.focus.row_index = 0;
    state.to_previous_pattern_row();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 0);

    state.focus.row_index = 1;
    state.to_next_pattern_row();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 2);

    state.to_next_pattern_row();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 0);

    state.focus.row_index = 1;
    state.to_next_pattern_row();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 1);
  }

  #[test]
  fn home_end_navigation_targets_pattern_columns_and_song_boundaries() {
    let mut state = test_state();
    state.patterns.get_mut(0).unwrap().num_rows = NonZero::new(3).unwrap();
    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: vec![Track { notes: vec![] }],
      num_rows: NonZero::new(2).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });
    state
      .sequence
      .titles
      .extend(["Second pattern".to_owned(), "Pattern".to_owned()]);
    state.sequence.end_of_song = 1;
    state.sequence.focus_index = 1;
    state.focus.row_index = 1;
    state.focus.track_column = TrackColumn::Velocity;

    state.to_first_pattern_column();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 1);
    assert_eq!(state.focus.track_index, 0);
    assert_eq!(state.focus.track_column, TrackColumn::Note);

    state.to_last_pattern_column();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 1);
    assert_eq!(state.focus.track_index, 0);
    assert_eq!(state.focus.track_column, TrackColumn::Effect);

    state.to_first_song_row();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 0);
    assert_eq!(state.focus.track_column, TrackColumn::Effect);

    state.to_last_song_or_pool_row();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 1);
    assert_eq!(state.focus.track_column, TrackColumn::Effect);

    state.to_last_song_or_pool_row();
    assert_eq!(state.sequence.focus_index, 2);
    assert_eq!(state.focus.row_index, 2);
  }

  #[test]
  fn page_navigation_seeks_pattern_boundaries_before_changing_patterns() {
    let mut state = test_state();
    state.patterns.get_mut(0).unwrap().num_rows = NonZero::new(3).unwrap();
    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: vec![Track { notes: vec![] }],
      num_rows: NonZero::new(2).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    });
    state.sequence.titles.push("Second pattern".to_owned());
    state.focus.row_index = 2;
    state.focus.track_column = TrackColumn::Velocity;

    state.page_up_pattern();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 0);

    state.page_up_pattern();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 0);

    state.page_down_pattern();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 2);

    state.page_down_pattern();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 1);
    assert_eq!(state.focus.track_column, TrackColumn::Velocity);

    state.page_down_pattern();
    assert_eq!(state.sequence.focus_index, 1);

    state.page_up_pattern();
    assert_eq!(state.sequence.focus_index, 1);
    assert_eq!(state.focus.row_index, 0);

    state.page_up_pattern();
    assert_eq!(state.sequence.focus_index, 0);
    assert_eq!(state.focus.row_index, 0);
  }

  #[test]
  fn focused_note_adjustments_use_editor_setting_steps() {
    let mut state = test_state();
    state.instruments = Instruments::DirectMapped(
      (0..=10)
        .map(|index| InstrumentMetadata {
          name: InstrumentName(index.to_string()),
          velocity_boundaries: vec![],
        })
        .collect(),
    );

    state.focus.track_column = TrackColumn::Note;
    state.adjust_focused_note(true, true);
    assert_eq!(focused_semitone(&state), Semitone(1));
    state.adjust_focused_note(false, true);
    assert_eq!(focused_semitone(&state), Semitone(0));
    state.adjust_focused_note(false, true);
    assert_eq!(focused_semitone(&state), Semitone(11));
    assert_eq!(focused_octave(&state), Octave(2));
    state.adjust_focused_note(true, true);
    assert_eq!(focused_semitone(&state), Semitone(0));
    assert_eq!(focused_octave(&state), Octave(3));
    state.set_focused_note(Note {
      octave: Octave(3),
      semitone: Semitone(11),
      instrument: InstrumentIndex(0),
      velocity: Velocity(50),
      effect: None,
    });
    state.adjust_focused_note(true, true);
    assert_eq!(focused_semitone(&state), Semitone(0));
    assert_eq!(focused_octave(&state), Octave(4));
    state.adjust_focused_note(true, false);
    assert_eq!(focused_octave(&state), Octave(5));
    state.adjust_focused_note(false, false);
    assert_eq!(focused_octave(&state), Octave(4));

    state.focus.track_column = TrackColumn::Instrument;
    state.adjust_focused_note(true, false);
    assert_eq!(focused_instrument(&state), InstrumentIndex(8));
    state.adjust_focused_note(true, false);
    assert_eq!(focused_instrument(&state), InstrumentIndex(10));

    state.focus.track_column = TrackColumn::Velocity;
    state.adjust_focused_note(true, false);
    assert_eq!(focused_velocity(&state), Velocity(60));
    state.adjust_focused_note(false, true);
    assert_eq!(focused_velocity(&state), Velocity(59));
  }

  #[test]
  fn focused_note_bounds_match_the_focused_column() {
    let mut state = test_state();
    state.instruments = Instruments::DirectMapped(
      (0..=10)
        .map(|index| InstrumentMetadata {
          name: InstrumentName(index.to_string()),
          velocity_boundaries: vec![],
        })
        .collect(),
    );

    state.focus.track_column = TrackColumn::Note;
    state.set_focused_note_to_bound(false);
    assert_eq!(focused_octave(&state), Octave(0));
    assert_eq!(focused_semitone(&state), Semitone(0));
    state.set_focused_note_to_bound(true);
    assert_eq!(focused_octave(&state), Octave(Note::MAX_OCTAVE));
    assert_eq!(focused_semitone(&state), Semitone(Note::MAX_SEMITONE));

    state.focus.track_column = TrackColumn::Instrument;
    state.set_focused_note_to_bound(false);
    assert_eq!(focused_instrument(&state), InstrumentIndex(0));
    state.set_focused_note_to_bound(true);
    assert_eq!(focused_instrument(&state), InstrumentIndex(10));

    state.focus.track_column = TrackColumn::Velocity;
    state.set_focused_note_to_bound(false);
    assert_eq!(focused_velocity(&state), Velocity(0));
    state.set_focused_note_to_bound(true);
    assert_eq!(focused_velocity(&state), Velocity(Velocity::MAX));
  }

  #[test]
  fn focused_instrument_digits_pair_within_one_second() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(4, start);
    assert_eq!(focused_instrument(&state), InstrumentIndex(4));

    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(999));
    assert_eq!(focused_instrument(&state), InstrumentIndex(42));
  }

  #[test]
  fn focused_instrument_digits_keep_only_the_latest_two() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(1, start);
    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(500));
    state.set_focused_note_instrument_digit(3, start + Duration::from_millis(999));

    assert_eq!(focused_instrument(&state), InstrumentIndex(23));
  }

  #[test]
  fn focused_instrument_digit_starts_over_after_one_second() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(4, start);
    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(1001));

    assert_eq!(focused_instrument(&state), InstrumentIndex(2));
  }

  #[test]
  fn focused_velocity_digits_keep_only_the_latest_two() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_velocity_digit(1, start);
    assert_eq!(focused_velocity(&state), Velocity(1));

    state.set_focused_note_velocity_digit(2, start + Duration::from_millis(500));
    state.set_focused_note_velocity_digit(3, start + Duration::from_millis(999));

    assert_eq!(focused_velocity(&state), Velocity(23));
  }

  #[test]
  fn focused_velocity_digit_starts_over_after_one_second() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_velocity_digit(4, start);
    state.set_focused_note_velocity_digit(2, start + Duration::from_millis(1001));

    assert_eq!(focused_velocity(&state), Velocity(2));
  }
}
