use std::num::{NonZero, NonZeroU8};

use nonempty::NonEmpty;

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
  pub pattern_row_index: usize,
  pub pattern_track_index: usize,
  pub track_column: TrackColumn,
}
