use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// A one-line horizontal rule that fills the available render width.
#[derive(Clone, Copy, Default)]
pub struct HorizontalRule;

impl Widget for HorizontalRule {
  fn render(self, area: Rect, buf: &mut Buffer) {
    if area.height == 0 {
      return;
    }

    for offset in 0..area.width {
      buf[(area.x + offset, area.y)].set_char('─');
    }
  }
}
