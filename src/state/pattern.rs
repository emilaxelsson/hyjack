use std::num::{NonZero, NonZeroU8};

use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};

use crate::state::instruments::InstrumentIndex;
use crate::utils::{Adjustable, nonzero_len, sub_one};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Octave(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Semitone(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Velocity(pub u8);

impl Octave {
  const LARGE_STEP: u8 = 2;
}

impl Velocity {
  const LARGE_STEP: u8 = 10;
  pub const MAX: u8 = 99;
}

impl Adjustable for Octave {
  type Context = ();

  fn increase(&mut self, _context: (), small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };
    self.0 = (self.0 + step).min(Note::MAX_OCTAVE);
  }

  fn decrease(&mut self, _context: (), small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };
    self.0 = self.0.saturating_sub(step);
  }
}

impl Adjustable for Velocity {
  type Context = ();

  fn increase(&mut self, _context: (), small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };
    self.0 = (self.0 + step).min(Self::MAX);
  }

  fn decrease(&mut self, _context: (), small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };
    self.0 = self.0.saturating_sub(step);
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
  pub octave: Octave,
  pub semitone: Semitone,
  pub instrument: InstrumentIndex,
  pub velocity: Velocity,
  #[serde(default)]
  pub effect: Option<Effect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
  SetTempo(NonZeroU8),
}

impl Note {
  pub const MAX_OCTAVE: u8 = 8;
  pub const MAX_SEMITONE: u8 = 11;
}

impl Adjustable for Note {
  type Context = ();

  fn increase(&mut self, _context: (), small_step: bool) {
    if small_step {
      if self.semitone.0 < Self::MAX_SEMITONE {
        self.semitone.0 += 1;
      } else if self.octave.0 < Self::MAX_OCTAVE {
        self.semitone.0 = 0;
        self.octave.0 += 1;
      }
    } else {
      self.octave.0 = (self.octave.0 + 1).min(Self::MAX_OCTAVE);
    }
  }

  fn decrease(&mut self, _context: (), small_step: bool) {
    if small_step {
      if self.semitone.0 > 0 {
        self.semitone.0 -= 1;
      } else if self.octave.0 > 0 {
        self.semitone.0 = Self::MAX_SEMITONE;
        self.octave.0 -= 1;
      }
    } else {
      self.octave.0 = self.octave.0.saturating_sub(1);
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Track {
  pub name: String,
  pub notes: NonEmpty<Option<Note>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Pattern {
  pub title: String,
  pub tracks: NonEmpty<Track>,
  pub num_rows: NonZero<usize>,
  pub rows_per_beat: NonZeroU8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PatternDefaults {
  pub num_rows: NonZero<usize>,
  pub track_titles: NonEmpty<String>,
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

  pub fn set_note(&mut self, row_index: usize, track_index: usize, note: Note) {
    if row_index >= self.num_rows.get() || track_index >= self.tracks.len() {
      return;
    }
    let track = &mut self.tracks[track_index];
    track.notes[row_index] = Some(note);
  }
}

impl Default for PatternDefaults {
  fn default() -> Self {
    Self {
      num_rows: NonZero::new(16).unwrap(),
      track_titles: NonEmpty::new("Track 1".to_owned()),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    }
  }
}

impl PatternDefaults {
  pub fn make_pattern(&self, title: String) -> Pattern {
    let mut notes = NonEmpty::new(None);
    for _ in 1..self.num_rows.get() {
      notes.push(None);
    }

    let mut track_titles = self.track_titles.iter();
    let first_track_title = track_titles.next().unwrap();
    let mut tracks = NonEmpty::new(Track {
      name: first_track_title.clone(),
      notes: notes.clone(),
    });
    for track_title in track_titles {
      tracks.push(Track {
        name: track_title.clone(),
        notes: notes.clone(),
      });
    }

    Pattern {
      title,
      tracks,
      num_rows: self.num_rows,
      rows_per_beat: self.rows_per_beat,
    }
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

  /// Jump a number of rows.
  pub fn jump_rows(&mut self, step: u8, bounds: PatternBounds) {
    let next_row = self.row_index + step as usize;
    if next_row < bounds.num_rows.get() {
      self.row_index = next_row;
    }
  }

  /// Navigate to the next row.
  pub fn to_next_row(&mut self, bounds: PatternBounds) {
    self.jump_rows(1, bounds);
  }

  /// Navigate to the previous track while retaining the focused column, or the first column.
  pub fn to_previous_track(&mut self) {
    if self.track_index > 0 {
      self.track_index -= 1;
    } else {
      self.track_column = TrackColumn::Note;
    }
  }

  /// Navigate to the next track while retaining the focused column, or the last column.
  pub fn to_next_track(&mut self, bounds: PatternBounds) {
    if self.track_index + 1 < bounds.num_tracks.get() {
      self.track_index += 1;
    } else {
      self.track_column = TrackColumn::Effect;
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

pub fn midi_pitch_from_octave(octave: Octave, semitone: Semitone) -> u8 {
  octave.0.saturating_add(1).saturating_mul(12) + semitone.0
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

  fn note_strategy() -> impl Strategy<Value = Note> {
    (
      any::<u8>(),
      0u8..12u8,
      any::<u8>().prop_map(InstrumentIndex),
      any::<u8>(),
    )
      .prop_map(|(octave, semitone, instrument, velocity)| Note {
        octave: Octave(octave),
        semitone: Semitone(semitone),
        instrument,
        velocity: Velocity(velocity),
        effect: None,
      })
  }

  fn empty_pattern(num_rows: usize, num_tracks: usize) -> Pattern {
    let mut notes = NonEmpty::new(None);
    for _ in 1..num_rows {
      notes.push(None);
    }

    let mut tracks = NonEmpty::new(Track {
      name: "Track 1".to_owned(),
      notes: notes.clone(),
    });
    for track_index in 1..num_tracks {
      tracks.push(Track {
        name: format!("Track {}", track_index + 1),
        notes: notes.clone(),
      });
    }

    Pattern {
      title: "Pattern".to_owned(),
      tracks,
      num_rows: NonZero::new(num_rows).unwrap(),
      rows_per_beat: NonZeroU8::new(4).unwrap(),
    }
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

    #[test]
    fn set_note_then_get_note_returns_written_note(
      (num_rows, num_tracks) in (1usize..=64, 1usize..=16),
      note in note_strategy(),
      row_seed in any::<usize>(),
      track_seed in any::<usize>(),
    ) {
      let row_index = row_seed % num_rows;
      let track_index = track_seed % num_tracks;

      let mut pattern = empty_pattern(num_rows, num_tracks);
      let expected_octave = note.octave;
      let expected_semitone = note.semitone;
      let expected_instrument = note.instrument;
      let expected_velocity = note.velocity;

      pattern.set_note(row_index, track_index, note);

      match pattern.get_note(row_index, track_index) {
        Some(saved_note) => {
          prop_assert_eq!(saved_note.octave, expected_octave);
          prop_assert_eq!(saved_note.semitone, expected_semitone);
          prop_assert_eq!(saved_note.instrument, expected_instrument);
          prop_assert_eq!(saved_note.velocity, expected_velocity);
        }
        None => prop_assert!(false, "expected Some(note) after set_note"),
      }
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
  fn track_navigation_moves_between_tracks_without_changing_column() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 1,
      track_column: TrackColumn::Velocity,
    };

    focus.to_previous_track();
    assert_eq!(focus.track_index, 0);
    assert_eq!(focus.track_column, TrackColumn::Velocity);

    focus.to_next_track(bounds(8, 3));
    assert_eq!(focus.track_index, 1);
    assert_eq!(focus.track_column, TrackColumn::Velocity);
  }

  #[test]
  fn track_navigation_jumps_to_pattern_columns_at_boundaries() {
    let mut focus = Focus {
      row_index: 0,
      track_index: 0,
      track_column: TrackColumn::Instrument,
    };

    focus.to_previous_track();
    assert_eq!(focus.track_index, 0);
    assert_eq!(focus.track_column, TrackColumn::Note);

    focus.track_index = 2;
    focus.to_next_track(bounds(8, 3));
    assert_eq!(focus.track_index, 2);
    assert_eq!(focus.track_column, TrackColumn::Effect);
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

  #[test]
  fn midi_pitch_from_octave_matches_tracker_conversion() {
    assert_eq!(midi_pitch_from_octave(Octave(3), Semitone(0)), 48);
    assert_eq!(midi_pitch_from_octave(Octave(3), Semitone(11)), 59);
    assert_eq!(midi_pitch_from_octave(Octave(8), Semitone(11)), 119);
  }

  #[test]
  fn note_adjustments_change_semitones_and_octaves() {
    let mut note = Note {
      octave: Octave(3),
      semitone: Semitone(11),
      instrument: InstrumentIndex(0),
      velocity: Velocity(50),
      effect: None,
    };

    note.increase((), true);
    assert_eq!((note.octave, note.semitone), (Octave(4), Semitone(0)));
    note.decrease((), true);
    assert_eq!((note.octave, note.semitone), (Octave(3), Semitone(11)));
    note.decrease((), true);
    assert_eq!((note.octave, note.semitone), (Octave(3), Semitone(10)));
    note.increase((), true);
    assert_eq!((note.octave, note.semitone), (Octave(3), Semitone(11)));
    note.increase((), false);
    assert_eq!((note.octave, note.semitone), (Octave(4), Semitone(11)));
    note.decrease((), false);
    assert_eq!((note.octave, note.semitone), (Octave(3), Semitone(11)));
  }
}
