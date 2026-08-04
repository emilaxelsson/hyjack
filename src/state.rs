use std::num::{NonZero, NonZeroU8};

use anyhow::Result;
use nonempty::NonEmpty;

use crate::utils::{nonzero_len, sub_one};

pub struct Note {
  pub pitch: u8,
  pub instrument: u8,
  pub velocity: u8,
}

pub struct Track {
  pub name: String,
  pub notes: NonEmpty<Option<Note>>,
}

pub struct Pattern {
  pub title: String,
  pub tracks: NonEmpty<Track>,
  pub beats: NonZeroU8,
  pub rpb: NonZeroU8,
}

pub struct PlayerTime {
  pub hour: u8,
  pub minute: u8,
  pub second: u8,
  pub millis: u16,
}

pub struct PlayerState {
  pub time: PlayerTime,
  pub sequence_index: usize,
}

pub enum TrackFocus {
  Note,
  Instrument,
  Velocity,
  Effect,
}

pub struct PatternScrollOffset {
  pub rows: usize,
  pub columns: usize,
}

impl TrackFocus {
  fn next(&self) -> Self {
    match self {
      Self::Note => Self::Instrument,
      Self::Instrument => Self::Velocity,
      Self::Velocity => Self::Effect,
      Self::Effect => Self::Note,
    }
  }

  fn previous(&self) -> Self {
    match self {
      Self::Note => Self::Effect,
      Self::Instrument => Self::Note,
      Self::Velocity => Self::Instrument,
      Self::Effect => Self::Velocity,
    }
  }
}

pub struct Focus {
  pub sequence_index: usize,
  pub pattern_index: usize,
  pub pattern_row_index: usize,
  pub pattern_track_index: usize,
  pub track_focus: TrackFocus,
  pub pattern_scroll_offset: PatternScrollOffset,
}

pub struct AppState {
  pub dracker_version: String,
  pub song_title: String,
  pub patterns: NonEmpty<Pattern>,
  pub sequence: Vec<String>,
  pub focus: Focus,
  pub player: PlayerState,
}

impl Default for AppState {
  fn default() -> Self {
    default_state()
  }
}

impl Pattern {
  pub fn number_of_rows(&self) -> NonZero<usize> {
    NonZero::new((self.beats.get() as usize) * (self.rpb.get() as usize)).unwrap()
  }
}

/// Trim and validate a user-facing song or pattern name.
///
/// # Examples
///
/// ```rust
/// use dracker::state::normalize_and_validate_name;
///
/// assert_eq!(
///   normalize_and_validate_name("Pattern title", "  Verse 1 A  ").unwrap(),
///   "Verse 1 A"
/// );
///
/// assert_eq!(
///   normalize_and_validate_name("Pattern title", "Verse@1")
///     .unwrap_err()
///     .to_string(),
///   "Pattern title contains invalid characters"
/// );
/// ```
pub fn normalize_and_validate_name(what: &str, name: &str) -> Result<String> {
  let trimmed_name = name.trim();
  if trimmed_name.is_empty() {
    anyhow::bail!("{} cannot be empty", what);
  }
  if trimmed_name.len() > 30 {
    anyhow::bail!("{} cannot be longer than 30 characters", what);
  }
  if !trimmed_name
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || ch == ' ' || ch == '_' || ch == '-')
  {
    anyhow::bail!("{} contains invalid characters", what);
  }
  Ok(trimmed_name.to_string())
}

impl AppState {
  pub fn rename_song(&mut self, new_title: String) -> Result<()> {
    let normalized = normalize_and_validate_name("Song title", &new_title)?;
    self.song_title = normalized;
    Ok(())
  }

  pub fn get_current_pattern(&self) -> &Pattern {
    &self.patterns[self.focus.pattern_index]
  }

  pub fn get_current_pattern_mut(&mut self) -> &mut Pattern {
    &mut self.patterns[self.focus.pattern_index]
  }

  /// Ensure that the pattern cursor (row and column) is within the bounds of the current
  /// pattern.
  pub fn refocus_pattern_cursor(&mut self) {
    let current_pattern = self.get_current_pattern();
    let number_of_rows = current_pattern.number_of_rows();
    let number_of_tracks = nonzero_len(&current_pattern.tracks);
    if self.focus.pattern_row_index >= number_of_rows.get() {
      self.focus.pattern_row_index = sub_one(number_of_rows);
    }
    if self.focus.pattern_track_index >= number_of_tracks.get() {
      self.focus.pattern_track_index = sub_one(number_of_tracks);
    }
  }

  /// Update the pattern index in the focus based on the current sequence index.
  pub fn update_pattern_index(&mut self) {
    let title = self.sequence[self.focus.sequence_index].clone();
    self.focus.pattern_index = self
      .patterns
      .iter()
      .position(|p| p.title == title)
      .unwrap_or(0);
  }

  /// Move to the next pattern in the sequence.
  pub fn to_next_pattern(&mut self) {
    if self.focus.sequence_index + 1 < self.sequence.len() {
      self.focus.sequence_index += 1;
      self.update_pattern_index();
      self.refocus_pattern_cursor();
    }
  }

  /// Move to the previous pattern in the sequence.
  pub fn to_previous_pattern(&mut self) {
    if self.focus.sequence_index > 0 {
      self.focus.sequence_index -= 1;
      self.update_pattern_index();
      self.refocus_pattern_cursor();
    }
  }

  /// Move to the previous row in the current pattern.
  pub fn to_previous_row(&mut self) {
    if self.focus.pattern_row_index > 0 {
      self.focus.pattern_row_index -= 1;
    }
  }

  /// Move to the next row in the current pattern.
  pub fn to_next_row(&mut self) {
    let current_pattern = self.get_current_pattern();
    let number_of_rows = current_pattern.number_of_rows();
    let next_row = self.focus.pattern_row_index + 1;
    if next_row < number_of_rows.get() {
      self.focus.pattern_row_index = next_row;
    }
  }

  /// Move to the previous editable field in the current pattern row.
  pub fn to_previous_track_field(&mut self) {
    match self.focus.track_focus {
      TrackFocus::Note => {
        if self.focus.pattern_track_index > 0 {
          self.focus.pattern_track_index -= 1;
          self.focus.track_focus = self.focus.track_focus.previous();
        }
      }
      _ => {
        self.focus.track_focus = self.focus.track_focus.previous();
      }
    }
  }

  /// Move to the next editable field in the current pattern row.
  pub fn to_next_track_field(&mut self) {
    let current_pattern = self.get_current_pattern();
    let number_of_tracks = nonzero_len(&current_pattern.tracks);

    match self.focus.track_focus {
      TrackFocus::Effect => {
        if self.focus.pattern_track_index + 1 < number_of_tracks.get() {
          self.focus.pattern_track_index += 1;
          self.focus.track_focus = self.focus.track_focus.next();
        }
      }
      _ => {
        self.focus.track_focus = self.focus.track_focus.next();
      }
    }
  }

  pub fn rename_current_pattern(&mut self, new_title: String) -> Result<()> {
    let normalized = normalize_and_validate_name("Pattern title", &new_title)?;
    let old_title = self.get_current_pattern().title.clone();
    let current_index = self.focus.pattern_index;

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

    self.get_current_pattern_mut().title = normalized.clone();

    self.sequence.iter_mut().for_each(|title| {
      if *title == old_title {
        *title = normalized.clone();
      }
    });

    Ok(())
  }
}

pub fn default_state() -> AppState {
  fn empty_notes_8() -> NonEmpty<Option<Note>> {
    NonEmpty::from_vec(std::iter::repeat_with(|| None).take(8).collect()).unwrap()
  }

  fn empty_track(name: &str) -> Track {
    Track {
      name: name.to_string(),
      notes: empty_notes_8(),
    }
  }

  AppState {
    dracker_version: "0.2".to_string(),
    song_title: "My song".to_string(),
    patterns: NonEmpty::from_vec(vec![
      Pattern {
        title: "Intro A".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Intro B".to_string(),
        tracks: NonEmpty::from_vec(vec![
          Track {
            name: "Hat/ride".to_string(),
            notes: NonEmpty::from_vec(vec![
              Some(Note {
                pitch: 36,
                instrument: 2,
                velocity: 20,
              }),
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
            ])
            .unwrap(),
          },
          Track {
            name: "Kick".to_string(),
            notes: NonEmpty::from_vec(vec![
              None, None, None, None, None, None, None, None, None, None, None, None, None, None,
              None, None,
            ])
            .unwrap(),
          },
          Track {
            name: "Snare".to_string(),
            notes: NonEmpty::from_vec(vec![
              None,
              None,
              None,
              None,
              Some(Note {
                pitch: 36,
                instrument: 2,
                velocity: 20,
              }),
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
              None,
            ])
            .unwrap(),
          },
          Track {
            name: "Cymbals".to_string(),
            notes: NonEmpty::from_vec(vec![
              None, None, None, None, None, None, None, None, None, None, None, None, None, None,
              None, None,
            ])
            .unwrap(),
          },
        ])
        .unwrap(),
        beats: NonZeroU8::new(4).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 A".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 B".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 C".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 D".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Bridge A".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Bridge B".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        beats: NonZeroU8::new(2).unwrap(),
        rpb: NonZeroU8::new(4).unwrap(),
      },
    ])
    .unwrap(),
    sequence: vec![
      "Intro A".to_string(),
      "Intro B".to_string(),
      "Verse 1 A".to_string(),
      "Verse 1 B".to_string(),
      "Verse 1 C".to_string(),
      "Verse 1 D".to_string(),
      "Intro B".to_string(),
      "Bridge A".to_string(),
      "Bridge B".to_string(),
    ],
    focus: Focus {
      sequence_index: 1,
      pattern_index: 1,
      pattern_row_index: 0,
      pattern_track_index: 0,
      track_focus: TrackFocus::Note,
      pattern_scroll_offset: PatternScrollOffset {
        rows: 0,
        columns: 0,
      },
    },
    player: PlayerState {
      time: PlayerTime {
        hour: 0,
        minute: 2,
        second: 32,
        millis: 0,
      },
      sequence_index: 0,
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_validate_name() {
    assert_eq!(
      normalize_and_validate_name("Name", "Valid Name 123_-").unwrap(),
      "Valid Name 123_-"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "  Valid Name 123_-  ").unwrap(),
      "Valid Name 123_-"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "")
        .unwrap_err()
        .to_string(),
      "Name cannot be empty"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "   ")
        .unwrap_err()
        .to_string(),
      "Name cannot be empty"
    );
    assert_eq!(
      normalize_and_validate_name("Name", "Invalid@Name")
        .unwrap_err()
        .to_string(),
      "Name contains invalid characters"
    );
    assert_eq!(
      normalize_and_validate_name(
        "Name",
        "ThisNameIsWayTooLongToBeValidBecauseItExceedsThirtyCharacters"
      )
      .unwrap_err()
      .to_string(),
      "Name cannot be longer than 30 characters"
    );
  }

  #[test]
  fn test_move_to_next_track_field_advances_track_after_effect() {
    let mut state = default_state();
    state.focus.pattern_track_index = 1;
    state.focus.track_focus = TrackFocus::Note;

    state.to_next_track_field();
    assert!(matches!(state.focus.track_focus, TrackFocus::Instrument));
    assert_eq!(state.focus.pattern_track_index, 1);

    state.to_next_track_field();
    assert!(matches!(state.focus.track_focus, TrackFocus::Velocity));
    assert_eq!(state.focus.pattern_track_index, 1);

    state.to_next_track_field();
    assert!(matches!(state.focus.track_focus, TrackFocus::Effect));
    assert_eq!(state.focus.pattern_track_index, 1);

    state.to_next_track_field();
    assert!(matches!(state.focus.track_focus, TrackFocus::Note));
    assert_eq!(state.focus.pattern_track_index, 2);
  }

  #[test]
  fn test_move_to_previous_track_field_moves_to_previous_track_note() {
    let mut state = default_state();
    state.focus.pattern_track_index = 1;
    state.focus.track_focus = TrackFocus::Note;

    state.to_previous_track_field();

    assert!(matches!(state.focus.track_focus, TrackFocus::Effect));
    assert_eq!(state.focus.pattern_track_index, 0);
  }

  #[test]
  fn test_move_to_previous_track_field_stops_on_first_track() {
    let mut state = default_state();
    state.focus.pattern_track_index = 0;
    state.focus.track_focus = TrackFocus::Note;

    state.to_previous_track_field();

    assert!(matches!(state.focus.track_focus, TrackFocus::Note));
    assert_eq!(state.focus.pattern_track_index, 0);
  }

  #[test]
  fn test_move_to_next_track_field_stops_on_last_track() {
    let mut state = default_state();
    state.focus.pattern_track_index = 3;
    state.focus.track_focus = TrackFocus::Effect;

    state.to_next_track_field();

    assert!(matches!(state.focus.track_focus, TrackFocus::Effect));
    assert_eq!(state.focus.pattern_track_index, 3);
  }
}
