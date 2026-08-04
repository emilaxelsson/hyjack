//! Helpers for rendering a widget inside a centered percentage-based area.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Widget;

/// Wraps another widget and renders it inside a rectangle centered within the parent
/// area. The inner rectangle is computed as a percentage of the available width and
/// height.
pub struct Centered<W> {
  percent_x: u16,
  percent_y: u16,
  widget: W,
}

fn centered_area(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
  let [_, inner, _] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage((100 - percent_y) / 2),
      Constraint::Percentage(percent_y),
      Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

  let [_, inner, _] = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Percentage((100 - percent_x) / 2),
      Constraint::Percentage(percent_x),
      Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(inner);

  inner
}

impl<W> Centered<W> {
  /// Create a centered wrapper for `widget`.
  ///
  /// `percent_x` and `percent_y` control how much of the parent area the inner widget may
  /// occupy horizontally and vertically.
  pub fn new(percent_x: u16, percent_y: u16, widget: W) -> Self {
    Self {
      percent_x,
      percent_y,
      widget,
    }
  }

  /// Returns the centered inner area this wrapper would use for rendering.
  pub fn inner_area(&self, area: Rect) -> Rect {
    centered_area(self.percent_x, self.percent_y, area)
  }
}

impl<W: Widget> Widget for Centered<W> {
  fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
    let inner = centered_area(self.percent_x, self.percent_y, area);
    self.widget.render(inner, buf);
  }
}
