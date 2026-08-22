use std::io;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::{Position, Rect, Size};

pub enum ComponentAction {
  None,
  Close,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EditorSettingFocus {
  Octave,
  Velocity,
  Step,
  Instrument,
}

#[derive(Clone, Copy, PartialEq)]
pub enum UIMode {
  EditPattern,
  EditSequence,
  EditSetting(EditorSettingFocus),
}

pub trait Component {
  type State;

  fn draw(&self, frame: &mut Frame);
  fn handle_event(&mut self, event: Event, state: &mut Self::State) -> io::Result<ComponentAction>;
}

pub trait AdjustableArea {
  /// Moves the area's left edge to the right by `x` units, reducing its width accordingly.
  fn sequeeze_right(self, x: u16) -> Rect;

  /// Moves the area's right edge to the left by `x` units, reducing its width accordingly.
  fn squeeze_left(self, x: u16) -> Rect;

  /// Moves the area's top edge down by `y` units, reducing its height accordingly.
  fn squeeze_down(self, y: u16) -> Rect;

  /// Moves the area's bottom edge up by `y` units, reducing its height accordingly.
  fn squeeze_up(self, y: u16) -> Rect;
}

impl AdjustableArea for Rect {
  fn sequeeze_right(self, x: u16) -> Rect {
    Rect {
      x: self.x.saturating_sub(x),
      width: self.width.saturating_sub(x),
      ..self
    }
  }

  fn squeeze_left(self, x: u16) -> Rect {
    Rect {
      width: self.width.saturating_sub(x),
      ..self
    }
  }

  fn squeeze_down(self, y: u16) -> Rect {
    Rect {
      y: self.y.saturating_add(y),
      height: self.height.saturating_sub(y),
      ..self
    }
  }

  fn squeeze_up(self, y: u16) -> Rect {
    Rect {
      height: self.height.saturating_sub(y),
      ..self
    }
  }
}

/// Convert a logical focus position relative to a scrollable content area into a position
/// relative to the visible viewport.
pub fn get_absolute_position(
  area: Rect,
  scroll_offset: Position,
  relative_position: Position,
) -> Position {
  Position {
    x: area.x + relative_position.x.saturating_sub(scroll_offset.x),
    y: area.y + relative_position.y.saturating_sub(scroll_offset.y),
  }
}

/// Create a `Rect` with the given size, positioned at the origin (0, 0).
pub fn rect_from_origin(size: Size) -> Rect {
  Rect {
    x: 0,
    y: 0,
    width: size.width,
    height: size.height,
  }
}
