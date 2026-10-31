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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstrumentMetadata {
  /// Used for display purposes only.
  pub name: InstrumentName,

  /// Velocity boundaries for the instrument's layers. The boundaries must be in the range
  /// (0.0, 1.0) and in ascending order.
  ///
  /// Some instruments may have a rather course velocity resolution. For example, a snare
  /// drum may be recorded with five distinct velocity layers. Assuming these velocities
  /// are evenly distributed, the boundaries would be `[0.2, 0.4, 0.6, 0.8]`.
  ///
  /// An empty vector indicates that the instrument has full velocity resolution.
  pub velocity_boundaries: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
pub enum Instruments {
  /// Instruments ordered according to their index. The same index is used both in the
  /// Hyjack editor and in the MIDI program mapping.
  DirectMapped(Vec<InstrumentMetadata>),
}

impl Instruments {
  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    match self {
      Instruments::DirectMapped(instruments) => instruments
        .get(index.0 as usize)
        .map(|instrument| instrument.name.clone()),
    }
  }

  pub fn max_index(&self) -> Option<InstrumentIndex> {
    match self {
      Instruments::DirectMapped(instruments) => {
        if instruments.is_empty() {
          None
        } else {
          Some(InstrumentIndex(
            (instruments.len() - 1).min(u8::MAX as usize) as u8,
          ))
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn direct_mapped_instruments_return_metadata_names() {
    let instruments = Instruments::DirectMapped(vec![
      InstrumentMetadata {
        name: InstrumentName("Snare".to_owned()),
        velocity_boundaries: vec![],
      },
      InstrumentMetadata {
        name: InstrumentName("Kick".to_owned()),
        velocity_boundaries: vec![],
      },
    ]);

    assert_eq!(
      instruments.get_name(InstrumentIndex(0)),
      Some(InstrumentName("Snare".to_owned()))
    );
    assert_eq!(
      instruments.get_name(InstrumentIndex(1)),
      Some(InstrumentName("Kick".to_owned()))
    );
    assert_eq!(instruments.get_name(InstrumentIndex(2)), None);
  }
}
