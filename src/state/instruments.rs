use serde::{Deserialize, Serialize};

use crate::utils::Adjustable;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentIndex(pub u8);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentName(pub String);

impl InstrumentIndex {
  const MAX: u8 = 99;
  const LARGE_STEP: u8 = 8;
}

impl Adjustable for InstrumentIndex {
  /// Maximum allowed instrument index.
  type Context = InstrumentIndex;

  fn increase(&mut self, max_instrument: InstrumentIndex, small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };

    // `max_instrument` should typically not exceed `MAX`, but we cap by `Self::MAX` to
    // ensure that we don't ever get more than two digits.
    self.0 = (self.0 + step).min(Self::MAX).min(max_instrument.0);
  }

  fn decrease(&mut self, _max_instrument: InstrumentIndex, small_step: bool) {
    let step = if small_step { 1 } else { Self::LARGE_STEP };
    self.0 = self.0.saturating_sub(step);
  }
}

#[derive(Serialize, Deserialize)]
pub enum Instruments {
  DirectMapped(Vec<InstrumentName>),
}

impl Instruments {
  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    match self {
      Instruments::DirectMapped(names) => names.get(index.0 as usize).cloned(),
    }
  }

  pub fn max_index(&self) -> Option<InstrumentIndex> {
    match self {
      Instruments::DirectMapped(names) => {
        if names.is_empty() {
          None
        } else {
          Some(InstrumentIndex(
            (names.len() - 1).min(u8::MAX as usize) as u8
          ))
        }
      }
    }
  }
}
