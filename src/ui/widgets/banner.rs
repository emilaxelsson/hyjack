use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

/// A full-width banner.
#[derive(Debug, Clone)]
pub struct Banner<'a> {
  text: &'a str,
  style: Style,
}

impl<'a> Banner<'a> {
  pub fn new(text: &'a str) -> Self {
    Self {
      text,
      style: Style::default(),
    }
  }

  #[must_use = "method moves the value of self and returns the modified value"]
  pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
    self.style = style.into();
    self
  }
}

impl Widget for Banner<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let area = area.intersection(buf.area);

    // Cap height to 1
    let area = area.intersection(Rect { height: 1, ..area });

    if area.is_empty() {
      return;
    }

    buf.set_style(area, self.style);

    // Clear the entire row to avoid leftover characters from previous renders.
    for x in area.left()..area.right() {
      buf[(x, area.y)].set_char(' ');
    }

    buf.set_stringn(area.x, area.y, self.text, area.width as usize, self.style);
  }
}
