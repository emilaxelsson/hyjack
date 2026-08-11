use std::num::{NonZero, NonZeroU8};

use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::state::instruments::InstrumentIndex;
use crate::utils::{nonzero_len, sub_one};

#[derive(Clone, Serialize, Deserialize)]
pub struct Note {
  pub pitch: u8,
  pub instrument: InstrumentIndex,
  pub velocity: u8,
}

#[derive(Serialize, Deserialize)]
pub struct Track {
  pub name: String,
  pub notes: NonEmpty<Option<Note>>,
}

#[derive(Serialize, Deserialize)]
pub struct Pattern {
  pub title: String,
  pub tracks: NonEmpty<Track>,
  pub num_rows: NonZero<usize>,
  pub rows_per_beat: NonZeroU8,
}

#[derive(Clone, Copy, Debug)]
pub struct PatternBounds {
  pub num_rows: NonZero<usize>,
  pub num_tracks: NonZero<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrackColumn {
  Note,
  Instrument,
  Velocity,
  Effect,
}

impl TrackColumn {
  pub fn next(&self) -> Self {
    match self {
      Self::Note => Self::Instrument,
      Self::Instrument => Self::Velocity,
      Self::Velocity => Self::Effect,
      Self::Effect => Self::Note,
    }
  }

  pub fn previous(&self) -> Self {
    match self {
      Self::Note => Self::Effect,
      Self::Instrument => Self::Note,
      Self::Velocity => Self::Instrument,
      Self::Effect => Self::Velocity,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Focus {
  pub row_index: usize,
  pub track_index: usize,
  pub track_column: TrackColumn,
}

impl Pattern {
  pub fn bounds(&self) -> PatternBounds {
    PatternBounds {
      num_rows: self.num_rows,
      num_tracks: nonzero_len(&self.tracks),
    }
  }

  pub fn get_note(&self, row_index: usize, track_index: usize) -> Option<Note> {
    if row_index >= self.num_rows.get() || track_index >= self.tracks.len() {
      return None;
    }
    let track = &self.tracks[track_index];
    track.notes[row_index].clone()
  }
}

impl Focus {
  /// Check if the focus is within the bounds of the given pattern.
  pub fn within_bounds(&self, bounds: &PatternBounds) -> bool {
    self.row_index < bounds.num_rows.get() && self.track_index < bounds.num_tracks.get()
  }

  /// Ensure that the focus is within the bounds of the given pattern.
  pub fn refocus(&mut self, bounds: &PatternBounds) {
    if self.row_index >= bounds.num_rows.get() {
      self.row_index = sub_one(bounds.num_rows);
    }
    if self.track_index >= bounds.num_tracks.get() {
      self.track_index = sub_one(bounds.num_tracks);
    }
  }

  /// Navigate to the previous row.
  pub fn to_previous_row(&mut self) {
    if self.row_index > 0 {
      self.row_index -= 1;
    }
  }

  /// Navigate to the next row.
  pub fn to_next_row(&mut self, bounds: PatternBounds) {
    let next_row = self.row_index + 1;
    if next_row < bounds.num_rows.get() {
      self.row_index = next_row;
    }
  }

  /// Navigate to the previous editable field in the current pattern row.
  pub fn to_previous_track_field(&mut self) {
    match self.track_column {
      TrackColumn::Note => {
        if self.track_index > 0 {
          self.track_index -= 1;
          self.track_column = self.track_column.previous();
        }
      }
      _ => {
        self.track_column = self.track_column.previous();
      }
    }
  }

  /// Navigate to the next editable field in the current pattern row.
  pub fn to_next_track_field(&mut self, bounds: PatternBounds) {
    match self.track_column {
      TrackColumn::Effect => {
        if self.track_index + 1 < bounds.num_tracks.get() {
          self.track_index += 1;
          self.track_column = self.track_column.next();
        }
      }
      _ => {
        self.track_column = self.track_column.next();
      }
    }
  }
}

#[cfg(test)]
mod focus_tests {
  use std::num::NonZero;

  use proptest::prelude::*;

  use super::*;

  fn bounds(rows: usize, tracks: usize) -> PatternBounds {
    PatternBounds {
      num_rows: NonZero::new(rows).unwrap(),
      num_tracks: NonZero::new(tracks).unwrap(),
    }
  }

  fn bounds_strategy(max_rows: usize, max_tracks: usize) -> impl Strategy<Value = PatternBounds> {
    (1..=max_rows, 1..=max_tracks).prop_map(|(rows, tracks)| bounds(rows, tracks))
  }

  fn track_column_strategy() -> impl Strategy<Value = TrackColumn> {
    prop_oneof![
      Just(TrackColumn::Note),
      Just(TrackColumn::Instrument),
      Just(TrackColumn::Velocity),
      Just(TrackColumn::Effect),
    ]
  }

  fn focus_strategy(num_rows: usize, num_tracks: usize) -> impl Strategy<Value = Focus> {
    (0..num_rows, 0..num_tracks, track_column_strategy()).prop_map(
      move |(row_index, track_index, track_column)| Focus {
        row_index,
        track_index,
        track_column,
      },
    )
  }

  proptest! {
    #[test]
    fn refocus_keeps_focus_within_bounds(
      (mut focus, bounds) in (focus_strategy(10, 10), bounds_strategy(15, 15))
    ) {
      prop_assume!(!focus.within_bounds(&bounds));

      focus.refocus(&bounds);

      prop_assert!(focus.within_bounds(&bounds));
    }

    #[test]
    fn refocus_does_not_change_focus_when_already_in_bounds(
      (mut focus, bounds) in bounds_strategy(20, 20).prop_flat_map(|bounds| {
        let focus = focus_strategy(bounds.num_rows.get(), bounds.num_tracks.get());
        (focus, Just(bounds))
      })
    ) {
      prop_assert!(focus.within_bounds(&bounds));

      let before = focus.clone();

      focus.refocus(&bounds);

      prop_assert_eq!(focus, before);
    }
  }

  #[test]
  fn to_previous_row_moves_up_when_not_on_first_row() {
    let mut focus = Focus {
      row_index: 3,
      track_index: 0,
      track_column: TrackColumn::Note,
    };

    focus.to_previous_row();

    assert_eq!(focus.row_index, 2);
  }

  #[test]
  fn to_previous_row_stays_on_first_row() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 0,
      track_column: TrackColumn::Instrument,
    };

    focus.to_previous_row();

    assert_eq!(focus.row_index, 0);
  }

  #[test]
  fn to_next_row_moves_down_when_not_on_last_row() {
    let mut focus = Focus {
      row_index: 1,
      track_index: 0,
      track_column: TrackColumn::Velocity,
    };

    focus.to_next_row(bounds(4, 2));

    assert_eq!(focus.row_index, 2);
  }

  #[test]
  fn to_next_row_stays_on_last_row() {
    let mut focus = Focus {
      row_index: 3,
      track_index: 0,
      track_column: TrackColumn::Effect,
    };

    focus.to_next_row(bounds(4, 2));

    assert_eq!(focus.row_index, 3);
  }

  #[test]
  fn to_previous_track_field_moves_to_previous_column_same_track() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 1,
      track_column: TrackColumn::Velocity,
    };

    focus.to_previous_track_field();

    assert_eq!(focus.track_index, 1);
    assert_eq!(focus.track_column, TrackColumn::Instrument);
  }

  #[test]
  fn to_previous_track_field_wraps_to_previous_track_effect() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 2,
      track_column: TrackColumn::Note,
    };

    focus.to_previous_track_field();

    assert_eq!(focus.track_index, 1);
    assert_eq!(focus.track_column, TrackColumn::Effect);
  }

  #[test]
  fn to_previous_track_field_stays_at_first_note_field() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 0,
      track_column: TrackColumn::Note,
    };

    focus.to_previous_track_field();

    assert_eq!(focus.track_index, 0);
    assert_eq!(focus.track_column, TrackColumn::Note);
  }

  #[test]
  fn to_next_track_field_moves_to_next_column_same_track() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 1,
      track_column: TrackColumn::Instrument,
    };

    focus.to_next_track_field(bounds(8, 4));

    assert_eq!(focus.track_index, 1);
    assert_eq!(focus.track_column, TrackColumn::Velocity);
  }

  #[test]
  fn to_next_track_field_wraps_to_next_track_note() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 1,
      track_column: TrackColumn::Effect,
    };

    focus.to_next_track_field(bounds(8, 4));

    assert_eq!(focus.track_index, 2);
    assert_eq!(focus.track_column, TrackColumn::Note);
  }

  #[test]
  fn to_next_track_field_stays_at_last_effect_field() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 2,
      track_column: TrackColumn::Effect,
    };

    focus.to_next_track_field(bounds(8, 3));

    assert_eq!(focus.track_index, 2);
    assert_eq!(focus.track_column, TrackColumn::Effect);
  }
}
