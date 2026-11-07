use std::io::{self, Write};
use std::num::NonZero;

use nonempty::NonEmpty;
use serde::Serialize;
use serde_json::value::RawValue;

pub fn sub_one(x: NonZero<usize>) -> usize {
  x.get() - 1
}

pub fn nonzero_len<T>(items: &NonEmpty<T>) -> NonZero<usize> {
  NonZero::new(items.len()).unwrap()
}

pub trait NonEmptyExt<T> {
  fn remove(&mut self, index: usize) -> Option<T>;
  fn swap(&mut self, first: usize, second: usize);
}

impl<T> NonEmptyExt<T> for NonEmpty<T> {
  fn remove(&mut self, index: usize) -> Option<T> {
    if self.len() == 1 {
      return None;
    }

    if index == 0 {
      let new_head = self.tail.remove(0);
      Some(std::mem::replace(&mut self.head, new_head))
    } else {
      Some(self.tail.remove(index - 1))
    }
  }

  fn swap(&mut self, first: usize, second: usize) {
    match (first, second) {
      (0, 0) => {}
      (0, tail_index) => std::mem::swap(&mut self.head, &mut self.tail[tail_index - 1]),
      (head_index, 0) => std::mem::swap(&mut self.tail[head_index - 1], &mut self.head),
      (first_tail_index, second_tail_index) => {
        self.tail.swap(first_tail_index - 1, second_tail_index - 1);
      }
    }
  }
}

/// Return an unused copy name derived from `name`.
pub fn fresh_name<'a>(name: &str, existing_names: impl IntoIterator<Item = &'a str>) -> String {
  let existing_names: Vec<_> = existing_names.into_iter().collect();
  let base_name = name
    .strip_suffix(" copy")
    .or_else(|| {
      name.rsplit_once(" copy").and_then(|(base, suffix)| {
        (!suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())).then_some(base)
      })
    })
    .unwrap_or(name);

  for copy_number in 0.. {
    let candidate = if copy_number == 0 {
      format!("{base_name} copy")
    } else {
      format!("{base_name} copy{copy_number}")
    };

    if !existing_names.contains(&candidate.as_str()) {
      return candidate;
    }
  }

  unreachable!()
}

pub trait Adjustable {
  type Context;

  fn increase(&mut self, context: Self::Context, small_step: bool);
  fn decrease(&mut self, context: Self::Context, small_step: bool);

  fn adjust(&mut self, context: Self::Context, increase: bool, small_step: bool) {
    if increase {
      self.increase(context, small_step);
    } else {
      self.decrease(context, small_step);
    }
  }
}

pub fn to_spaced_raw_value<T: Serialize>(value: &T) -> serde_json::Result<Box<RawValue>> {
  let mut json = Vec::new();
  let mut serializer = serde_json::Serializer::with_formatter(&mut json, SpacedFormatter);
  value.serialize(&mut serializer)?;
  serde_json::from_slice(&json)
}

struct SpacedFormatter;

impl serde_json::ser::Formatter for SpacedFormatter {
  fn begin_array_value<W: ?Sized + Write>(
    &mut self,
    writer: &mut W,
    first: bool,
  ) -> io::Result<()> {
    if first {
      Ok(())
    } else {
      writer.write_all(b", ")
    }
  }

  fn begin_object_key<W: ?Sized + Write>(&mut self, writer: &mut W, first: bool) -> io::Result<()> {
    if first {
      Ok(())
    } else {
      writer.write_all(b", ")
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fresh_name_uses_the_first_available_copy_name() {
    let names = ["Foo", "Foo copy", "Foo copy1"];

    assert_eq!(fresh_name("Foo", names), "Foo copy2");
    assert_eq!(fresh_name("Foo copy1", names), "Foo copy2");
  }

  #[test]
  fn remove_removes_head_and_tail_items() {
    let mut items = NonEmpty::from_vec(vec![1, 2, 3]).unwrap();

    assert_eq!(items.remove(0), Some(1));
    assert_eq!(items, NonEmpty::from_vec(vec![2, 3]).unwrap());

    assert_eq!(items.remove(1), Some(3));
    assert_eq!(items, NonEmpty::new(2));

    assert_eq!(items.remove(0), None);
    assert_eq!(items, NonEmpty::new(2));
  }

  #[test]
  fn swap_exchanges_head_and_tail_items() {
    let mut items = NonEmpty::from_vec(vec![1, 2, 3]).unwrap();

    items.swap(0, 2);
    assert_eq!(items, NonEmpty::from_vec(vec![3, 2, 1]).unwrap());

    items.swap(1, 2);
    assert_eq!(items, NonEmpty::from_vec(vec![3, 1, 2]).unwrap());
  }

  #[test]
  fn spaced_formatter_preserves_string_commas_and_formats_nested_values() {
    let value = serde_json::json!({"values": ["a,b", {"first": 1, "second": 2}]});

    let raw = to_spaced_raw_value(&value).unwrap();

    assert_eq!(raw.get(), r#"{"values":["a,b", {"first":1, "second":2}]}"#);
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(raw.get()).unwrap(),
      value
    );
  }
}
