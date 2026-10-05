use std::num::NonZero;

use nonempty::NonEmpty;

pub fn sub_one(x: NonZero<usize>) -> usize {
  x.get() - 1
}

pub fn nonzero_len<T>(items: &NonEmpty<T>) -> NonZero<usize> {
  NonZero::new(items.len()).unwrap()
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fresh_name_uses_the_first_available_copy_name() {
    let names = ["Foo", "Foo copy", "Foo copy1"];

    assert_eq!(fresh_name("Foo", names), "Foo copy2");
    assert_eq!(fresh_name("Foo copy1", names), "Foo copy2");
  }
}
