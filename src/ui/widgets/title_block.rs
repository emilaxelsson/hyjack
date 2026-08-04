use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Widget;

/// A lightweight block-like widget with no borders and a full-width title row.
#[derive(Debug, Clone, Default)]
pub struct TitleBlock<'a> {
  title: Option<Line<'a>>,
  style: Style,
  title_style: Style,
  title_alignment: Alignment,
}

impl<'a> TitleBlock<'a> {
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use = "method moves the value of self and returns the modified value"]
  pub fn title<T: Into<Line<'a>>>(mut self, title: T) -> Self {
    self.title = Some(title.into());
    self
  }

  #[must_use = "method moves the value of self and returns the modified value"]
  pub fn title_style<S: Into<Style>>(mut self, style: S) -> Self {
    self.title_style = style.into();
    self
  }

  #[must_use = "method moves the value of self and returns the modified value"]
  pub fn title_alignment(mut self, alignment: Alignment) -> Self {
    self.title_alignment = alignment;
    self
  }

  #[must_use = "method moves the value of self and returns the modified value"]
  pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
    self.style = style.into();
    self
  }

  pub fn inner(&self, area: Rect) -> Rect {
    if self.title.is_some() {
      Rect {
        y: area.y.saturating_add(1).min(area.bottom()),
        height: area.height.saturating_sub(1),
        ..area
      }
    } else {
      area
    }
  }

  fn title_area(&self, area: Rect) -> Rect {
    Rect {
      height: u16::from(area.height > 0),
      ..area
    }
  }
}

impl Widget for TitleBlock<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
      return;
    }

    buf.set_style(area, self.style);

    let title_area = self.title_area(area);

    let Some(title) = self.title else {
      return;
    };

    buf.set_style(title_area, self.title_style);

    let aligned_title = match self.title_alignment {
      Alignment::Left => title.left_aligned(),
      Alignment::Center => title.centered(),
      Alignment::Right => title.right_aligned(),
    };
    aligned_title.render(title_area, buf);
  }
}
