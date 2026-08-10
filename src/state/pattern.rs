use std::num::{NonZero, NonZeroU8};

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
  pub num_rows: NonZero<usize>,
  pub rows_per_beat: NonZeroU8,
}

pub struct PatternBounds {
  pub num_rows: NonZero<usize>,
  pub num_tracks: NonZero<usize>,
}

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
}

impl Focus {
  /// Ensure that the focus is within the bounds of the given pattern.
  pub fn refocus(&mut self, bounds: &PatternBounds) {
    if self.row_index >= bounds.num_rows.get() {
      self.row_index = sub_one(bounds.num_rows);
    }
    if self.track_index >= bounds.num_tracks.get() {
      self.track_index = sub_one(bounds.num_tracks);
    }
  }
}
