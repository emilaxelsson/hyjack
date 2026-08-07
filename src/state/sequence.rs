//! This module contains the `Sequence` struct, which represents a sequence of patterns in
//! a song.

#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
  pub pattern_refs: Vec<String>,
  pub focus_index: usize,
  /// Index of the last pattern in the sequence that is part of the song. Patterns after
  /// this index can be seen as a "pool" of patterns that can be used in the song.
  pub end_of_song: usize,
}

impl Sequence {
  /// Returns the number of patterns in the pool (i.e. the number of patterns after the
  /// end of the song).
  pub fn pool_size(&self) -> usize {
    self.pattern_refs.len().saturating_sub(self.end_of_song + 1)
  }

  /// Navigate to the next pattern.
  pub fn to_next_pattern(&mut self) {
    if self.focus_index + 1 < self.pattern_refs.len() {
      self.focus_index += 1;
    }
  }

  /// Navigate to the previous pattern.
  pub fn to_previous_pattern(&mut self) {
    if self.focus_index > 0 {
      self.focus_index -= 1;
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
        self.pattern_refs.swap(idx, idx - 1);
        self.focus_index = self.focus_index.saturating_sub(1);
      }
    }
  }

  /// Move the currently focused pattern down by one position.
  pub fn move_pattern_down(&mut self) {
    if self.focus_index + 1 < self.pattern_refs.len() || self.end_of_song == self.focus_index {
      if self.end_of_song == self.focus_index {
        self.end_of_song = self.end_of_song.saturating_sub(1);
      } else {
        let idx = self.focus_index;
        self.pattern_refs.swap(idx, idx + 1);
        self.focus_index += 1;
      }
    }
  }

  /// Duplicate the currently focused pattern reference just below the current position.
  pub fn duplicate_focused_pattern_ref(&mut self) {
    let idx = self.focus_index;
    let focused_pattern_ref = self.pattern_refs[idx].clone();
    self.pattern_refs.insert(idx + 1, focused_pattern_ref);
    self.focus_index += 1;

    // Keep the duplicated reference inside the song when duplicating a song entry.
    if idx <= self.end_of_song {
      self.end_of_song += 1;
    }
  }
}

#[cfg(test)]
mod tests {
  use proptest::prelude::*;

  use super::*;

  fn sequence_strategy(max_size: usize) -> impl Strategy<Value = Sequence> {
    (
      proptest::collection::vec(any::<u16>(), 1..max_size),
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
          pattern_refs: patterns,
          focus_index: focus_seed % len,
          end_of_song: end_seed % len,
        }
      })
  }

  // Condition for when the focused pattern cannot move up
  fn blocked_up(sequence: &Sequence) -> bool {
    sequence.focus_index == 0
  }

  // Condition for when the focused pattern cannot move down
  fn blocked_down(sequence: &Sequence) -> bool {
    let len = sequence.pattern_refs.len();
    let idx = sequence.focus_index;
    let end_of_song = sequence.end_of_song;
    let is_first_pattern = idx == 0;
    let is_last_pattern = idx + 1 == len;

    // The first pattern pattern cannot be moved past the end of the song, as that would
    // result in an empty song.
    (is_first_pattern && end_of_song == 0)
    ||
    // The last pattern can be moved down if the pool is empty. In that case, it moves
    // down to become part of the pool.
    (is_last_pattern && sequence.pool_size() != 0)
  }

  fn valid_move(original: &Sequence, sequence: &Sequence) -> bool {
    let idx_diff = sequence.focus_index as isize - original.focus_index as isize;
    let end_diff = sequence.end_of_song as isize - original.end_of_song as isize;
    let idx_change = idx_diff.abs() == 1 && end_diff == 0;
    let end_change = end_diff.abs() == 1 && idx_diff == 0;
    let same_length = sequence.pattern_refs.len() == original.pattern_refs.len();

    (idx_change || end_change) && same_length
  }

  proptest! {
    #[test]
    fn move_pattern_up_blocked_from_first_position(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = 0;
      let original_sequence = sequence.clone();

      sequence.move_pattern_up();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_pattern_down_blocked_from_last_position(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = sequence.pattern_refs.len() - 1;
      let original_sequence = sequence.clone();

      // The setting of `sequence.focus_index` above isn't enough to guarantee blockage,
      // but it prevents throwing away too many cases in the assumption below.
      prop_assume!(blocked_down(&sequence));

      sequence.move_pattern_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_pattern_down_first_item_blocked_when_song_ends_on_first(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = 0;
      sequence.end_of_song = 0;
      let original_sequence = sequence.clone();

      sequence.move_pattern_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_pattern_up_succeeds_when_not_blocked(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();

      prop_assume!(!blocked_up(&sequence));

      sequence.move_pattern_up();

      prop_assert!(valid_move(&original_sequence, &sequence));
    }

    #[test]
    fn move_pattern_down_succeeds_when_not_blocked(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();

      prop_assume!(!blocked_down(&sequence));

      sequence.move_pattern_down();

      prop_assert!(valid_move(&original_sequence, &sequence));
    }

    #[test]
    fn no_move_up_implies_blocked(
      mut sequence in sequence_strategy(6)
    ) {
      let original_sequence = sequence.clone();

      sequence.move_pattern_up();

      prop_assume!(sequence == original_sequence);
      prop_assert!(blocked_up(&sequence));
    }

    #[test]
    fn no_move_down_implies_blocked(
      mut sequence in sequence_strategy(6)
    ) {
      let original_sequence = sequence.clone();

      sequence.move_pattern_down();

      prop_assume!(sequence == original_sequence);
      prop_assert!(blocked_down(&sequence));
    }

    #[test]
    fn move_up_and_down_cancel_each_other(
      mut sequence in sequence_strategy(20)
    ) {
      let len = sequence.pattern_refs.len();
      let idx = sequence.focus_index;

      prop_assume!(idx > 0 && idx + 1 < len);

      let original_sequence = sequence.clone();

      sequence.move_pattern_up();
      sequence.move_pattern_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn duplicate_increases_length_and_moves_focus_to_inserted_item(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();
      let original_focus = original_sequence.focus_index;

      sequence.duplicate_focused_pattern_ref();

      prop_assert_eq!(
        sequence.pattern_refs.len(),
        original_sequence.pattern_refs.len() + 1
      );
      prop_assert_eq!(sequence.focus_index, original_focus + 1);
      prop_assert_eq!(
        sequence.pattern_refs[original_focus + 1].as_str(),
        original_sequence.pattern_refs[original_focus].as_str()
      );
    }

    #[test]
    fn duplicate_keeps_song_membership_of_original_reference(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();
      let original_focus = original_sequence.focus_index;
      let original_end = original_sequence.end_of_song;
      let focused_was_in_song = original_focus <= original_end;

      sequence.duplicate_focused_pattern_ref();

      prop_assert_eq!(
        sequence.end_of_song,
        original_end + usize::from(focused_was_in_song)
      );

      let duplicated_index = original_focus + 1;
      let duplicated_is_in_song = duplicated_index <= sequence.end_of_song;
      prop_assert_eq!(duplicated_is_in_song, focused_was_in_song);
    }
  }
}
