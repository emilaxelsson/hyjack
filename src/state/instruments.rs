use serde::{Deserialize, Serialize};

use crate::state::pattern::Velocity;
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
  /// Some instruments may have a rather coarse velocity resolution. For example, a snare
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

impl InstrumentMetadata {
  /// Returns the midpoint velocity of a zero-based layer, using 0.0 and 1.0 as the outer
  /// boundaries. The midpoint is scaled to 0-100, rounded to the nearest integer, and
  /// clamped to the supported velocity range (0-99).
  ///
  /// `layer` must be at most `velocity_boundaries.len()`.
  fn layer_velocity(&self, layer: usize) -> Velocity {
    let lower = if layer == 0 {
      0.0
    } else {
      self.velocity_boundaries[layer - 1]
    };
    let upper = self.velocity_boundaries.get(layer).copied().unwrap_or(1.0);
    Velocity(
      ((lower + upper) * 50.0)
        .round()
        .clamp(0.0, f64::from(Velocity::MAX)) as u8,
    )
  }

  /// Returns the closest layer midpoint to `velocity`, choosing the lower midpoint when
  /// two are equally close.
  fn snap_velocity(&self, velocity: Velocity) -> Velocity {
    (0..=self.velocity_boundaries.len())
      .map(|layer| self.layer_velocity(layer))
      .min_by_key(|candidate| candidate.0.abs_diff(velocity.0))
      .expect("an instrument always has at least one layer")
  }

  fn adjust_velocity(&self, velocity: Velocity, increase: bool) -> Velocity {
    let mut midpoints =
      (0..=self.velocity_boundaries.len()).map(|layer| self.layer_velocity(layer));
    if increase {
      midpoints
        .find(|midpoint| midpoint.0 > velocity.0)
        .unwrap_or_else(|| self.layer_velocity(self.velocity_boundaries.len()))
    } else {
      midpoints
        .rev()
        .find(|midpoint| midpoint.0 < velocity.0)
        .unwrap_or_else(|| self.layer_velocity(0))
    }
  }
}

impl Instruments {
  fn get(&self, index: InstrumentIndex) -> Option<&InstrumentMetadata> {
    match self {
      Instruments::DirectMapped(instruments) => instruments.get(index.0 as usize),
    }
  }

  pub fn get_name(&self, index: InstrumentIndex) -> Option<InstrumentName> {
    self.get(index).map(|instrument| instrument.name.clone())
  }

  pub fn snap_velocity(&self, index: InstrumentIndex, velocity: Velocity) -> Velocity {
    match self.get(index) {
      Some(instrument) if !instrument.velocity_boundaries.is_empty() => {
        instrument.snap_velocity(velocity)
      }
      _ => velocity,
    }
  }

  pub fn adjust_velocity(
    &self,
    index: InstrumentIndex,
    velocity: &mut Velocity,
    increase: bool,
    small_step: bool,
  ) {
    match self.get(index) {
      Some(instrument) if !instrument.velocity_boundaries.is_empty() => {
        *velocity = instrument.adjust_velocity(*velocity, increase);
      }
      _ => velocity.adjust((), increase, small_step),
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

  #[test]
  fn snapping_uses_nearest_layer_midpoint_and_breaks_ties_downward() {
    let instruments = Instruments::DirectMapped(vec![InstrumentMetadata {
      name: InstrumentName("Layered".to_owned()),
      velocity_boundaries: vec![0.2, 0.4, 0.8],
    }]);

    for (input, expected) in [
      (0, 10),
      (10, 10),
      (20, 10),
      (30, 30),
      (40, 30),
      (60, 60),
      (70, 60),
      (80, 90),
      (90, 90),
      (99, 90),
    ] {
      assert_eq!(
        instruments.snap_velocity(InstrumentIndex(0), Velocity(input)),
        Velocity(expected)
      );
    }

    assert_eq!(
      instruments.snap_velocity(InstrumentIndex(1), Velocity(70)),
      Velocity(70)
    );
  }

  #[test]
  fn adjustment_moves_to_next_midpoint_in_requested_direction() {
    let instruments = Instruments::DirectMapped(vec![InstrumentMetadata {
      name: InstrumentName("Layered".to_owned()),
      velocity_boundaries: vec![0.2, 0.4, 0.8],
    }]);

    for (input, increase, expected) in [
      (0, false, 10),
      (0, true, 10),
      (10, true, 30),
      (10, false, 10),
      (19, true, 30),
      (20, false, 10),
      (20, true, 30),
      (30, true, 60),
      (40, false, 30),
      (55, true, 60),
      (55, false, 30),
      (60, true, 90),
      (60, false, 30),
      (70, true, 90),
      (70, false, 60),
      (80, false, 60),
      (90, false, 60),
      (99, true, 90),
      (99, false, 90),
    ] {
      for small_step in [true, false] {
        let mut velocity = Velocity(input);
        instruments.adjust_velocity(InstrumentIndex(0), &mut velocity, increase, small_step);
        assert_eq!(velocity, Velocity(expected), "input {input}");
      }
    }
  }

  #[test]
  fn instruments_without_boundaries_use_full_velocity_resolution() {
    let instruments = Instruments::DirectMapped(vec![InstrumentMetadata {
      name: InstrumentName("Continuous".to_owned()),
      velocity_boundaries: vec![],
    }]);

    for index in [InstrumentIndex(0), InstrumentIndex(1)] {
      assert_eq!(instruments.snap_velocity(index, Velocity(70)), Velocity(70));
      let mut velocity = Velocity(50);
      instruments.adjust_velocity(index, &mut velocity, true, true);
      assert_eq!(velocity, Velocity(51));
      instruments.adjust_velocity(index, &mut velocity, false, false);
      assert_eq!(velocity, Velocity(41));
    }
  }
}
