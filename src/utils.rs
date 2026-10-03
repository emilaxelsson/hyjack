use std::num::NonZero;

use nonempty::NonEmpty;

pub fn sub_one(x: NonZero<usize>) -> usize {
  x.get() - 1
}

pub fn nonzero_len<T>(items: &NonEmpty<T>) -> NonZero<usize> {
  NonZero::new(items.len()).unwrap()
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
