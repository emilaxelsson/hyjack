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
use crate::state::instruments::Instruments;
use crate::state::name::normalize_and_validate_name;
use crate::state::pattern::{Focus, Note, Pattern, Velocity};
use crate::state::sequence::Sequence;

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

    note.instrument = instrument;
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
  use crate::state::instruments::InstrumentIndex;
  use crate::state::pattern::Track;

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
            instrument: 0,
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

  #[test]
  fn focused_instrument_digits_pair_within_one_second() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(4, start);
    assert_eq!(focused_instrument(&state), 4);

    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(999));
    assert_eq!(focused_instrument(&state), 42);
  }

  #[test]
  fn focused_instrument_digits_keep_only_the_latest_two() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(1, start);
    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(500));
    state.set_focused_note_instrument_digit(3, start + Duration::from_millis(999));

    assert_eq!(focused_instrument(&state), 23);
  }

  #[test]
  fn focused_instrument_digit_starts_over_after_one_second() {
    let mut state = test_state();
    let start = Instant::now();

    state.set_focused_note_instrument_digit(4, start);
    state.set_focused_note_instrument_digit(2, start + Duration::from_millis(1001));

    assert_eq!(focused_instrument(&state), 2);
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
