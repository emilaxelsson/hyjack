pub mod instruments;
pub mod name;
pub mod pattern;
pub mod sequence;

use std::num::{NonZero, NonZeroU8};
use std::path::Path;

use anyhow::Result;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::state::instruments::Instruments;
use crate::state::name::normalize_and_validate_name;
use crate::state::pattern::{Focus, Note, Pattern, Track, TrackColumn};
use crate::state::sequence::Sequence;

#[derive(Serialize)]
pub struct PlayerTime {
  pub hour: u8,
  pub minute: u8,
  pub second: u8,
  pub millis: u16,
}

#[derive(Serialize)]
pub struct PlayerState {
  pub time: PlayerTime,
  pub sequence_index: usize,
}

#[derive(Serialize)]
pub struct AppState {
  pub dracker_version: String,
  pub song_title: String,
  pub patterns: NonEmpty<Pattern>,
  pub sequence: Sequence,
  pub instruments: Instruments,
  pub focus: Focus,
  pub player: PlayerState,
}

impl Default for AppState {
  fn default() -> Self {
    default_state()
  }
}

impl AppState {
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
    let title = self.sequence.titles.get(self.sequence.focus_index).unwrap();

    self
      .patterns
      .iter()
      .position(|p| p.title == *title)
      .unwrap()
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
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
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
        num_rows: NonZero::new(16).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 A".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 B".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 C".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Verse 1 D".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Bridge A".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
      Pattern {
        title: "Bridge B".to_string(),
        tracks: NonEmpty::new(empty_track("Track 1")),
        num_rows: NonZero::new(8).unwrap(),
        rows_per_beat: NonZeroU8::new(4).unwrap(),
      },
    ])
    .unwrap(),
    sequence: Sequence {
      titles: vec![
        "Intro A".to_string(),
        "Intro B".to_string(),
        "Verse 1 A".to_string(),
        "Verse 1 B".to_string(),
        "Verse 1 C".to_string(),
        "Verse 1 D".to_string(),
        "Intro B".to_string(),
        "Bridge A".to_string(),
      ],
      focus_index: 1,
      end_of_song: 0,
    },
    instruments: Instruments::DirectMapped(vec![
      "Hihat".to_string(),
      "Ride".to_string(),
      "Kick".to_string(),
      "Snare".to_string(),
      "Crash".to_string(),
    ]),
    focus: Focus {
      row_index: 0,
      track_index: 0,
      track_column: TrackColumn::Note,
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
