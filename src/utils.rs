use std::num::NonZero;

use nonempty::NonEmpty;

pub fn sub_one(x: NonZero<usize>) -> usize {
  x.get() - 1
}

pub fn nonzero_len<T>(items: &NonEmpty<T>) -> NonZero<usize> {
  NonZero::new(items.len()).unwrap()
}
