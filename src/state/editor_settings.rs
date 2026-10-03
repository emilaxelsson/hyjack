use crate::state::instruments::InstrumentIndex;
use crate::state::pattern::{Note, Octave, Semitone, Velocity};

const LARGE_OCTAVE_STEP: u8 = 2;
const LARGE_VELOCITY_STEP: u8 = 10;
const LARGE_STEP_STEP: u8 = 4;
const LARGE_INSTRUMENT_STEP: u8 = 8;

const MAX_VELOCITY: u8 = 99;
const MAX_STEP: u8 = 16;
const MAX_INSTRUMENT: u8 = 99;

#[derive(Clone, Copy)]
pub struct EditorSettings {
  pub octave: Octave,
  pub velocity: Velocity,
  pub step: u8,
  pub instrument: InstrumentIndex,
}

impl Default for EditorSettings {
  fn default() -> Self {
    Self {
      octave: Octave(3),
      velocity: Velocity(50),
      step: 4,
      instrument: 0,
    }
  }
}

#[derive(Clone, Copy, PartialEq)]
pub enum EditorSettingFocus {
  Octave,
  Velocity,
  Step,
  Instrument,
}

impl EditorSettings {
  pub fn increase_octave(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_OCTAVE_STEP };
    self.octave.0 = (self.octave.0 + step).min(Note::MAX_OCTAVE);
  }

  pub fn decrease_octave(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_OCTAVE_STEP };
    self.octave.0 = self.octave.0.saturating_sub(step);
  }

  pub fn increase_velocity(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_VELOCITY_STEP };
    self.velocity.0 = (self.velocity.0 + step).min(MAX_VELOCITY);
  }

  pub fn decrease_velocity(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_VELOCITY_STEP };
    self.velocity.0 = self.velocity.0.saturating_sub(step);
  }

  pub fn increase_step(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_STEP_STEP };
    self.step = (self.step + step).min(MAX_STEP);
  }

  pub fn decrease_step(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_STEP_STEP };
    self.step = self.step.saturating_sub(step);
  }

  pub fn increase_instrument(&mut self, small_step: bool, max_instrument: u8) {
    let step = if small_step { 1 } else { LARGE_INSTRUMENT_STEP };

    // `max_instrument` should typically not exceed `MAX_INSTRUMENT`, but we cap by
    // `MAX_INSTRUMENT` to ensure that we don't ever get more than two digits.
    self.instrument = (self.instrument + step)
      .min(MAX_INSTRUMENT)
      .min(max_instrument);
  }

  pub fn decrease_instrument(&mut self, small_step: bool) {
    let step = if small_step { 1 } else { LARGE_INSTRUMENT_STEP };
    self.instrument = self.instrument.saturating_sub(step);
  }

  pub fn increase(&mut self, setting: EditorSettingFocus, small_step: bool, max_instrument: u8) {
    match setting {
      EditorSettingFocus::Octave => {
        self.increase_octave(small_step);
      }
      EditorSettingFocus::Velocity => {
        self.increase_velocity(small_step);
      }
      EditorSettingFocus::Step => {
        self.increase_step(small_step);
      }
      EditorSettingFocus::Instrument => {
        self.increase_instrument(small_step, max_instrument);
      }
    }
  }

  pub fn decrease(&mut self, setting: EditorSettingFocus, small_step: bool) {
    match setting {
      EditorSettingFocus::Octave => {
        self.decrease_octave(small_step);
      }
      EditorSettingFocus::Velocity => {
        self.decrease_velocity(small_step);
      }
      EditorSettingFocus::Step => {
        self.decrease_step(small_step);
      }
      EditorSettingFocus::Instrument => {
        self.decrease_instrument(small_step);
      }
    }
  }

  /// Create a `Note` based on the current editor settings and the given semitone.
  pub fn make_note(&self, semitone: Semitone) -> Note {
    Note {
      octave: self.octave,
      semitone,
      instrument: self.instrument,
      velocity: self.velocity,
    }
  }
}

impl EditorSettingFocus {
  pub fn next(self) -> Self {
    match self {
      Self::Octave => Self::Velocity,
      Self::Velocity => Self::Step,
      Self::Step => Self::Instrument,
      Self::Instrument => Self::Instrument,
    }
  }

  pub fn previous(self) -> Self {
    match self {
      Self::Octave => Self::Octave,
      Self::Velocity => Self::Octave,
      Self::Step => Self::Velocity,
      Self::Instrument => Self::Step,
    }
  }
}
