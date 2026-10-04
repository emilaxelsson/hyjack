pub mod editor_settings;
pub mod instruments;
pub mod name;
pub mod pattern;
pub mod sequence;

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::state::editor_settings::EditorSettings;
use crate::state::instruments::{InstrumentIndex, Instruments};
use crate::state::name::normalize_and_validate_name;
use crate::state::pattern::{Focus, Note, Pattern, TrackColumn, Velocity};
use crate::state::sequence::Sequence;
use crate::utils::Adjustable;

#[derive(Serialize, Deserialize)]
pub struct PlayerTime {
  pub hour: u8,
  pub minute: u8,
  pub second: u8,
  pub millis: u16,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerState {
  pub time: PlayerTime,
  pub sequence_index: usize,
}

#[derive(Serialize, Deserialize)]
pub struct AppState {
  pub dracker_version: String,
  pub song_title: String,
  pub patterns: NonEmpty<Pattern>,
  pub sequence: Sequence,
  pub instruments: Instruments,
  pub focus: Focus,
  pub player: PlayerState,
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
    let state = serde_json::from_str(&json)?;
    Ok(state)
  }

  pub fn save_to_file(&self, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(self)?;
    std::fs::write(path, json)?;
    Ok(())
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

  /// Ensure that the pattern focus is within the bounds of the current pattern.
  pub fn refocus_pattern(&mut self) {
    let current_pattern = self.get_current_pattern();
    self.focus.refocus(&current_pattern.bounds());
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
    let bounds = self.get_current_pattern().bounds();
    self.focus.row_index = bounds.num_rows.get() - 1;
    self.focus.refocus(&bounds);
  }

  /// Navigate to the next pattern row, crossing to the following sequence entry at the last row.
  pub fn to_next_pattern_row(&mut self) {
    let bounds = self.get_current_pattern().bounds();
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
    self.focus.track_index = self.get_current_pattern().bounds().num_tracks.get() - 1;
    self.focus.track_column = TrackColumn::Effect;
  }

  /// Navigate to the first row of the current pattern.
  pub fn to_first_pattern_row(&mut self) {
    self.focus.row_index = 0;
  }

  /// Navigate to the last row of the current pattern.
  pub fn to_last_pattern_row(&mut self) {
    self.focus.row_index = self.get_current_pattern().bounds().num_rows.get() - 1;
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
    self.focus.row_index = self.get_current_pattern().bounds().num_rows.get() - 1;
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
    if self.focus.row_index + 1 == self.get_current_pattern().bounds().num_rows.get() {
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

  /// Set the note at the current pattern focus.
  pub fn set_focused_note(&mut self, note: Note) {
    let pattern_index = self.get_current_pattern_index();
    let Some(pattern) = self.patterns.get_mut(pattern_index) else {
      return;
    };
    pattern.set_note(self.focus.row_index, self.focus.track_index, note);
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
    let current_pattern = self.get_current_pattern();
    let step = self.editor_settings.step;
    self.focus.jump_rows(step, current_pattern.bounds());
  }
}

#[cfg(test)]
mod tests {
  use std::num::{NonZero, NonZeroU8};

  use super::*;
  use crate::state::instruments::{InstrumentIndex, InstrumentName};
  use crate::state::pattern::{Octave, Semitone, Track};

  fn test_state() -> AppState {
    AppState {
      dracker_version: "test".to_owned(),
      song_title: "Test song".to_owned(),
      patterns: NonEmpty::new(Pattern {
        title: "Pattern".to_owned(),
        tracks: NonEmpty::new(Track {
          name: "Track".to_owned(),
          notes: NonEmpty::new(Some(Note {
            octave: crate::state::pattern::Octave(3),
            semitone: crate::state::pattern::Semitone(0),
            instrument: InstrumentIndex(0),
            velocity: Velocity(50),
          })),
        }),
        num_rows: NonZero::new(1).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      }),
      sequence: Sequence {
        titles: vec!["Pattern".to_owned()],
        focus_index: 0,
        end_of_song: 0,
      },
      instruments: Instruments::DirectMapped(vec![]),
      focus: Focus {
        row_index: 0,
        track_index: 0,
        track_column: crate::state::pattern::TrackColumn::Instrument,
      },
      player: PlayerState {
        time: PlayerTime {
          hour: 0,
          minute: 0,
          second: 0,
          millis: 0,
        },
        sequence_index: 0,
      },
      editor_settings: EditorSettings::default(),
      instrument_digit_entry: None,
      velocity_digit_entry: None,
    }
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
  fn pattern_row_navigation_crosses_sequence_entries_at_pattern_boundaries() {
    let mut state = test_state();
    state.patterns.get_mut(0).unwrap().num_rows = NonZero::new(3).unwrap();
    state.patterns.push(Pattern {
      title: "Second pattern".to_owned(),
      tracks: NonEmpty::new(Track {
        name: "Track".to_owned(),
        notes: NonEmpty::new(None),
      }),
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
      tracks: NonEmpty::new(Track {
        name: "Track".to_owned(),
        notes: NonEmpty::new(None),
      }),
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
      tracks: NonEmpty::new(Track {
        name: "Track".to_owned(),
        notes: NonEmpty::new(None),
      }),
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
        .map(|index| InstrumentName(index.to_string()))
        .collect(),
    );

    state.focus.track_column = crate::state::pattern::TrackColumn::Note;
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
    });
    state.adjust_focused_note(true, true);
    assert_eq!(focused_semitone(&state), Semitone(0));
    assert_eq!(focused_octave(&state), Octave(4));
    state.adjust_focused_note(true, false);
    assert_eq!(focused_octave(&state), Octave(5));
    state.adjust_focused_note(false, false);
    assert_eq!(focused_octave(&state), Octave(4));

    state.focus.track_column = crate::state::pattern::TrackColumn::Instrument;
    state.adjust_focused_note(true, false);
    assert_eq!(focused_instrument(&state), InstrumentIndex(8));
    state.adjust_focused_note(true, false);
    assert_eq!(focused_instrument(&state), InstrumentIndex(10));

    state.focus.track_column = crate::state::pattern::TrackColumn::Velocity;
    state.adjust_focused_note(true, false);
    assert_eq!(focused_velocity(&state), Velocity(60));
    state.adjust_focused_note(false, true);
    assert_eq!(focused_velocity(&state), Velocity(59));
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
