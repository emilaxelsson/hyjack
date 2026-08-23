use crate::state::instruments::InstrumentIndex;

const LARGE_OCTAVE_STEP: u8 = 2;
const LARGE_VELOCITY_STEP: u8 = 10;
const LARGE_STEP_STEP: u8 = 4;
const LARGE_INSTRUMENT_STEP: u8 = 8;

#[derive(Clone, Copy)]
pub struct EditorSettings {
  pub octave: u8,
  pub velocity: u8,
  pub step: u8,
  pub instrument: InstrumentIndex,
}

impl Default for EditorSettings {
  fn default() -> Self {
    Self {
      octave: 3,
      velocity: 50,
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
  pub fn increase(&mut self, setting: EditorSettingFocus, small_step: bool, max_instrument: u8) {
    let step = if small_step {
      1
    } else {
      match setting {
        EditorSettingFocus::Octave => LARGE_OCTAVE_STEP,
        EditorSettingFocus::Velocity => LARGE_VELOCITY_STEP,
        EditorSettingFocus::Step => LARGE_STEP_STEP,
        EditorSettingFocus::Instrument => LARGE_INSTRUMENT_STEP,
      }
    };

    match setting {
      EditorSettingFocus::Octave => {
        self.octave = self.octave.saturating_add(step);
      }
      EditorSettingFocus::Velocity => {
        self.velocity = self.velocity.saturating_add(step);
      }
      EditorSettingFocus::Step => {
        self.step = self.step.saturating_add(step);
      }
      EditorSettingFocus::Instrument => {
        self.instrument = self.instrument.saturating_add(step).min(max_instrument);
      }
    }
  }

  pub fn decrease(&mut self, setting: EditorSettingFocus, small_step: bool) {
    let step = if small_step {
      1
    } else {
      match setting {
        EditorSettingFocus::Octave => LARGE_OCTAVE_STEP,
        EditorSettingFocus::Velocity => LARGE_VELOCITY_STEP,
        EditorSettingFocus::Step => LARGE_STEP_STEP,
        EditorSettingFocus::Instrument => LARGE_INSTRUMENT_STEP,
      }
    };

    match setting {
      EditorSettingFocus::Octave => {
        self.octave = self.octave.saturating_sub(step);
      }
      EditorSettingFocus::Velocity => {
        self.velocity = self.velocity.saturating_sub(step);
      }
      EditorSettingFocus::Step => {
        self.step = self.step.saturating_sub(step);
      }
      EditorSettingFocus::Instrument => {
        self.instrument = self.instrument.saturating_sub(step);
      }
    }
  }
}

impl EditorSettingFocus {
  pub fn navigate_left(self) -> Self {
    match self {
      Self::Octave => Self::Octave,
      Self::Velocity => Self::Octave,
      Self::Step => Self::Velocity,
      Self::Instrument => Self::Step,
    }
  }

  pub fn navigate_right(self) -> Self {
    match self {
      Self::Octave => Self::Velocity,
      Self::Velocity => Self::Step,
      Self::Step => Self::Instrument,
      Self::Instrument => Self::Instrument,
    }
  }
}
