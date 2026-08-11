//! This module contains the `Sequence` struct, which is used to represent a sequence of
//! patterns in a song.

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(Clone))]
#[derive(Serialize, Deserialize)]
pub struct Sequence {
  pub titles: Vec<String>,
  pub focus_index: usize,
  /// Index of the last title in the sequence that is part of the song. Titles after this
  /// index can be seen as a "pool" of titles that can be used in the song.
  pub end_of_song: usize,
}

impl Sequence {
  /// Returns the number of titles in the pool (i.e. the number of titles after the end of
  /// the song).
  pub fn pool_size(&self) -> usize {
    self.titles.len().saturating_sub(self.end_of_song + 1)
  }

  /// Navigate to the next title.
  pub fn to_next_title(&mut self) {
    if self.focus_index + 1 < self.titles.len() {
      self.focus_index += 1;
    }
  }

  /// Navigate to the previous title.
  pub fn to_previous_title(&mut self) {
    if self.focus_index > 0 {
      self.focus_index -= 1;
    }
  }

  /// Move the focused title up by one position.
  pub fn move_title_up(&mut self) {
    if self.focus_index > 0 {
      if self.end_of_song + 1 == self.focus_index {
        // If the title is right after the end of the song, we move the end down instead
        // of moving the title up
        self.end_of_song += 1;
      } else {
        // Otherwise, move the title up
        let idx = self.focus_index;
        self.titles.swap(idx, idx - 1);
        self.focus_index = self.focus_index.saturating_sub(1);
      }
    }
  }

  /// Move the focused title down by one position.
  pub fn move_title_down(&mut self) {
    if self.focus_index + 1 < self.titles.len() || self.end_of_song == self.focus_index {
      if self.end_of_song == self.focus_index {
        self.end_of_song = self.end_of_song.saturating_sub(1);
      } else {
        let idx = self.focus_index;
        self.titles.swap(idx, idx + 1);
        self.focus_index += 1;
      }
    }
  }

  /// Duplicate the focused title just below the current position.
  pub fn duplicate_focused_title(&mut self) {
    let idx = self.focus_index;
    let focused_title = self.titles[idx].clone();
    self.titles.insert(idx + 1, focused_title);
    self.focus_index += 1;

    // Keep the duplicated title inside the song when duplicating a song entry.
    if idx <= self.end_of_song {
      self.end_of_song += 1;
    }
  }

  /// Insert a new title at the specified index.
  pub fn insert_title_at(&mut self, index: usize, title: String) {
    let idx = index.min(self.titles.len());
    self.titles.insert(idx, title);

    if idx <= self.focus_index {
      self.focus_index += 1;
    }

    if idx <= self.end_of_song {
      self.end_of_song += 1;
    }
  }

  /// Insert a new title right after the current focus.
  pub fn insert_title_at_focus(&mut self, title: String) {
    self.insert_title_at(self.focus_index + 1, title);
  }

  /// Soft delete the focused title. If it is the only occurrence, keep it by moving it to
  /// the end of the pool instead.
  pub fn soft_delete_focused_title(&mut self) {
    let idx = self.focus_index;

    if idx == 0 && self.end_of_song == 0 {
      // Don't allow deleting the first title if it is the only song title, as that would
      // result in an empty song.
      return;
    }

    let focused_title = self.titles[idx].clone();
    let title_in_song = idx <= self.end_of_song;
    let title_count = self
      .titles
      .iter()
      .filter(|title| **title == focused_title)
      .count();

    if title_count == 1 {
      if title_in_song {
        // Move to end of pool
        self.titles.remove(idx);
        self.titles.push(focused_title);
      }
    } else {
      self.titles.remove(idx);
      if self.focus_index >= self.titles.len() {
        self.focus_index = self.titles.len().saturating_sub(1);
      }
    }

    if title_in_song {
      self.end_of_song = self.end_of_song.saturating_sub(1);
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
      .prop_map(|(title_ids, focus_seed, end_seed)| {
        let titles: Vec<String> = title_ids
          .into_iter()
          .map(|id| format!("Title {id}"))
          .collect();
        let len = titles.len();

        Sequence {
          titles,
          focus_index: focus_seed % len,
          end_of_song: end_seed % len,
        }
      })
  }

  // Condition for when the focused title cannot move up
  fn blocked_up(sequence: &Sequence) -> bool {
    sequence.focus_index == 0
  }

  // Condition for when the focused title cannot move down
  fn blocked_down(sequence: &Sequence) -> bool {
    let len = sequence.titles.len();
    let idx = sequence.focus_index;
    let end_of_song = sequence.end_of_song;
    let is_first_title = idx == 0;
    let is_last_title = idx + 1 == len;

    // The first title cannot be moved past the end of the song, as that would result in
    // an empty song.
    (is_first_title && end_of_song == 0)
    ||
    // The last title can be moved down if the pool is empty. In that case, it moves down
    // to become part of the pool.
    (is_last_title && sequence.pool_size() != 0)
  }

  fn valid_move(original: &Sequence, sequence: &Sequence) -> bool {
    let idx_diff = sequence.focus_index as isize - original.focus_index as isize;
    let end_diff = sequence.end_of_song as isize - original.end_of_song as isize;
    let idx_change = idx_diff.abs() == 1 && end_diff == 0;
    let end_change = end_diff.abs() == 1 && idx_diff == 0;
    let same_length = sequence.titles.len() == original.titles.len();

    (idx_change || end_change) && same_length
  }

  fn title_count(sequence: &Sequence, title: &str) -> usize {
    sequence
      .titles
      .iter()
      .filter(|t| t.as_str() == title)
      .count()
  }

  proptest! {
    #[test]
    fn move_title_up_blocked_from_first_position(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = 0;
      let original_sequence = sequence.clone();

      sequence.move_title_up();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_title_down_blocked_from_last_position(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = sequence.titles.len() - 1;
      let original_sequence = sequence.clone();

      // The setting of `sequence.focus_index` above isn't enough to guarantee blockage,
      // but it prevents throwing away too many cases in the assumption below.
      prop_assume!(blocked_down(&sequence));

      sequence.move_title_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_title_down_first_item_blocked_when_song_ends_on_first(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = 0;
      sequence.end_of_song = 0;
      let original_sequence = sequence.clone();

      sequence.move_title_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn move_title_up_succeeds_when_not_blocked(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();

      prop_assume!(!blocked_up(&sequence));

      sequence.move_title_up();

      prop_assert!(valid_move(&original_sequence, &sequence));
    }

    #[test]
    fn move_title_down_succeeds_when_not_blocked(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();

      prop_assume!(!blocked_down(&sequence));

      sequence.move_title_down();

      prop_assert!(valid_move(&original_sequence, &sequence));
    }

    #[test]
    fn no_move_up_implies_blocked(
      mut sequence in sequence_strategy(6)
    ) {
      let original_sequence = sequence.clone();

      sequence.move_title_up();

      prop_assume!(sequence == original_sequence);
      prop_assert!(blocked_up(&sequence));
    }

    #[test]
    fn no_move_down_implies_blocked(
      mut sequence in sequence_strategy(6)
    ) {
      let original_sequence = sequence.clone();

      sequence.move_title_down();

      prop_assume!(sequence == original_sequence);
      prop_assert!(blocked_down(&sequence));
    }

    #[test]
    fn move_up_and_down_cancel_each_other(
      mut sequence in sequence_strategy(20)
    ) {
      let len = sequence.titles.len();
      let idx = sequence.focus_index;

      prop_assume!(idx > 0 && idx + 1 < len);

      let original_sequence = sequence.clone();

      sequence.move_title_up();
      sequence.move_title_down();

      prop_assert_eq!(sequence, original_sequence);
    }

    #[test]
    fn duplicate_increases_length_and_moves_focus_to_inserted_item(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();
      let original_focus = original_sequence.focus_index;

      sequence.duplicate_focused_title();

      prop_assert_eq!(
        sequence.titles.len(),
        original_sequence.titles.len() + 1
      );
      prop_assert_eq!(sequence.focus_index, original_focus + 1);
      prop_assert_eq!(
        sequence.titles[original_focus + 1].as_str(),
        original_sequence.titles[original_focus].as_str()
      );
    }

    #[test]
    fn duplicate_keeps_song_membership_of_original_title(
      mut sequence in sequence_strategy(20)
    ) {
      let original_sequence = sequence.clone();
      let original_focus = original_sequence.focus_index;
      let original_end = original_sequence.end_of_song;
      let focused_was_in_song = original_focus <= original_end;

      sequence.duplicate_focused_title();

      prop_assert_eq!(
        sequence.end_of_song,
        original_end + usize::from(focused_was_in_song)
      );

      let duplicated_index = original_focus + 1;
      let duplicated_is_in_song = duplicated_index <= sequence.end_of_song;
      prop_assert_eq!(duplicated_is_in_song, focused_was_in_song);
    }

    #[test]
    fn soft_delete_removes_focused_title_when_it_has_multiple_occurrences(
      (mut sequence, dup_index) in sequence_strategy(100).prop_flat_map(|sequence| {
        let len = sequence.titles.len();
        (Just(sequence), 0..len)
      })
    ) {
      let title = sequence.titles[sequence.focus_index].clone();

      // Ensure that the focused title has multiple occurrences
      sequence.insert_title_at(dup_index, title.clone());

      let length_before = sequence.titles.len();
      let count_before = title_count(&sequence, &title);
      prop_assert!(count_before > 1);
      let focus_before = sequence.focus_index;
      let was_last_entry = focus_before == sequence.titles.len() - 1;

      prop_assume!(!(sequence.focus_index == 0 && sequence.end_of_song == 0));

      sequence.soft_delete_focused_title();

      let length_after = sequence.titles.len();
      let count_after = title_count(&sequence, &title);
      let focus_after = sequence.focus_index;

      prop_assert_eq!(length_after, length_before - 1);
      prop_assert_eq!(count_after, count_before - 1);

      if was_last_entry {
        prop_assert_eq!(focus_after, length_after.saturating_sub(1));
      } else {
        prop_assert_eq!(focus_after, focus_before);
      }
    }

    #[test]
    fn soft_delete_does_not_move_from_song_when_song_has_only_one_entry(
      mut sequence in sequence_strategy(20)
    ) {
      sequence.focus_index = 0;
      sequence.end_of_song = 0;

      let before = sequence.clone();

      sequence.soft_delete_focused_title();

      prop_assert_eq!(sequence, before);
    }

    #[test]
    fn soft_delete_moves_single_title_from_song_to_end_of_pool(
      mut sequence in sequence_strategy(20)
    ) {
      let title = sequence.titles[sequence.focus_index].clone();

      // Ensure that the focused title is in the song
      prop_assume!(sequence.focus_index <= sequence.end_of_song);

      // Ensure that it's not the only entry in the song
      prop_assume!(!(sequence.focus_index == 0 && sequence.end_of_song == 0));

      // Ensure that it's the only occurrence of this title
      prop_assume!(title_count(&sequence, &title) == 1);

      let length_before = sequence.titles.len();

      sequence.soft_delete_focused_title();

      let length_after = sequence.titles.len();
      let count_after = title_count(&sequence, &title);
      let position_after = sequence.titles.iter().position(|t| t == &title).unwrap();

      prop_assert_eq!(length_after, length_before);
      prop_assert_eq!(count_after, 1);
      prop_assert_eq!(position_after, length_after - 1);
    }

    #[test]
    fn soft_delete_does_not_move_single_title_when_already_in_pool(
      mut sequence in sequence_strategy(20)
    ) {
      let title = sequence.titles[sequence.focus_index].clone();

      // Ensure that the focused title is in the pool
      prop_assume!(sequence.focus_index > sequence.end_of_song);

      // Ensure that it's the only occurrence of this title
      prop_assume!(title_count(&sequence, &title) == 1);

      let before = sequence.clone();

      sequence.soft_delete_focused_title();

      prop_assert_eq!(sequence, before);
    }

    #[test]
    fn duplicate_followed_by_soft_delete_does_nothing(
      mut sequence in sequence_strategy(20)
    ) {
      let before = sequence.clone();

      sequence.duplicate_focused_title();
      sequence.soft_delete_focused_title();

      prop_assert_eq!(sequence.titles, before.titles);
    }
  }
}
