//! This module contains the `Sequence` struct, which represents a sequence of patterns in
//! a song.

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
  pub patterns: Vec<String>,
  pub focus_index: usize,
  /// Index of the last pattern in the sequence that is part of the song. Patterns after
  /// this index can be seen as a "pool" of patterns that can be used in the song.
  pub end_of_song: usize,
}

impl Sequence {
  /// Move to the next pattern.
  pub fn to_next_pattern(&mut self) -> bool {
    if self.focus_index + 1 < self.patterns.len() {
      self.focus_index += 1;
      true
    } else {
      false
    }
  }

  /// Move to the previous pattern.
  pub fn to_previous_pattern(&mut self) -> bool {
    if self.focus_index > 0 {
      self.focus_index -= 1;
      true
    } else {
      false
    }
  }

  /// Move the currently focused pattern up by one position.
  pub fn move_pattern_up(&mut self) {
    if self.focus_index > 0 {
      if self.end_of_song + 1 == self.focus_index {
        // If the pattern is right after the end of the song, we move the end down instead
        // of moving the pattern up
        self.end_of_song += 1;
      } else {
        // Otherwise, move the pattern up
        let idx = self.focus_index;
        self.patterns.swap(idx, idx - 1);
        self.focus_index = self.focus_index.saturating_sub(1);
      }
    }
  }

  /// Move the currently focused pattern down by one position.
  pub fn move_pattern_down(&mut self) {
    if self.focus_index + 1 < self.patterns.len() {
      if self.end_of_song == self.focus_index {
        self.end_of_song = self.end_of_song.saturating_sub(1);
      } else {
        let idx = self.focus_index;
        self.patterns.swap(idx, idx + 1);
        self.focus_index += 1;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use proptest::prelude::*;

  use super::*;

  fn sequence_strategy() -> impl Strategy<Value = Sequence> {
    (
      proptest::collection::vec(any::<u16>(), 1..32),
      any::<usize>(),
      any::<usize>(),
    )
      .prop_map(|(pattern_ids, focus_seed, end_seed)| {
        let patterns: Vec<String> = pattern_ids
          .into_iter()
          .map(|id| format!("Pattern {id}"))
          .collect();
        let len = patterns.len();

        Sequence {
          patterns,
          focus_index: focus_seed % len,
          end_of_song: end_seed % len,
        }
      })
  }

  fn setup_sequence_state(focus_index: usize, end_of_song: usize) -> Sequence {
    let patterns = vec![
      "Intro A".to_string(),
      "Intro B".to_string(),
      "Verse 1 A".to_string(),
      "Verse 1 B".to_string(),
      "Verse 1 C".to_string(),
      "Verse 1 D".to_string(),
      "Intro B".to_string(),
      "Bridge A".to_string(),
    ];
    let end_of_song = end_of_song.min(patterns.len() - 1);
    let focus_index = focus_index.min(patterns.len() - 1);

    Sequence {
      patterns,
      focus_index,
      end_of_song,
    }
  }

  #[test]
  fn test_sequence_move_pattern_up_cannot_move_from_first_position() {
    let mut seq = setup_sequence_state(0, 3);
    let patterns_before = seq.patterns.clone();
    let end_of_song_before = seq.end_of_song;

    seq.move_pattern_up();

    assert_eq!(seq.focus_index, 0);
    assert_eq!(seq.patterns, patterns_before);
    assert_eq!(seq.end_of_song, end_of_song_before);
  }

  #[test]
  fn test_sequence_move_pattern_down_cannot_move_from_last_position() {
    let mut seq = setup_sequence_state(0, 3);
    let last_index = seq.patterns.len() - 1;
    seq.focus_index = last_index;
    let patterns_before = seq.patterns.clone();
    let end_of_song_before = seq.end_of_song;

    seq.move_pattern_down();

    assert_eq!(seq.focus_index, last_index);
    assert_eq!(seq.patterns, patterns_before);
    assert_eq!(seq.end_of_song, end_of_song_before);
  }

  #[test]
  fn test_sequence_move_pattern_down_first_item_blocked_when_song_ends_on_first() {
    let mut seq = setup_sequence_state(0, 0);
    let patterns_before = seq.patterns.clone();
    let end_of_song_before = seq.end_of_song;

    seq.move_pattern_down();

    assert_eq!(seq.focus_index, 0);
    assert_eq!(seq.patterns, patterns_before);
    assert_eq!(seq.end_of_song, end_of_song_before);
  }

  proptest! {
    #[test]
    fn prop_sequence_move_up_and_down_cancel_each_other(
      mut sequence in sequence_strategy()
    ) {
      let len = sequence.patterns.len();
      let idx = sequence.focus_index;

      prop_assume!(idx > 0 && idx + 1 < len);

      let original_sequence = sequence.clone();

      sequence.move_pattern_up();
      sequence.move_pattern_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn prop_sequence_length_does_not_change_when_move_is_allowed(
      mut sequence in sequence_strategy(),
      move_down in any::<bool>(),
    ) {
      let len = sequence.patterns.len();
      let idx = sequence.focus_index;
      let end_of_song = sequence.end_of_song;

      let edge_case = if move_down {
        idx + 1 >= len || (idx == 0 && end_of_song == 0)
      } else {
        idx == 0
      };

      prop_assume!(!edge_case);

      let before_len = sequence.patterns.len();

      if move_down {
        sequence.move_pattern_down();
      } else {
        sequence.move_pattern_up();
      }

      prop_assert_eq!(sequence.patterns.len(), before_len);
    }

    #[test]
    fn prop_song_and_pool_sizes_change_by_at_most_one_when_move_is_allowed(
      mut sequence in sequence_strategy(),
      move_down in any::<bool>(),
    ) {
      let len = sequence.patterns.len();
      let idx = sequence.focus_index;
      let end_of_song = sequence.end_of_song;

      let edge_case = if move_down {
        idx + 1 >= len || (idx == 0 && end_of_song == 0)
      } else {
        idx == 0
      };

      prop_assume!(!edge_case);

      let song_len_before = sequence.end_of_song + 1;
      let pool_len_before = sequence.patterns.len() - song_len_before;

      if move_down {
        sequence.move_pattern_down();
      } else {
        sequence.move_pattern_up();
      }

      let song_len_after = sequence.end_of_song + 1;
      let pool_len_after = sequence.patterns.len() - song_len_after;

      let song_diff = song_len_after.abs_diff(song_len_before);
      let pool_diff = pool_len_after.abs_diff(pool_len_before);

      prop_assert!(song_diff <= 1, "Song length changed by more than 1");
      prop_assert!(pool_diff <= 1, "Pool length changed by more than 1");
    }
  }
}
