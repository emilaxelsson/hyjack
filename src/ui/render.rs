mod head;
pub mod pattern;
mod sequence;
pub mod style;
mod track;

use ratatui::Frame;
use ratatui::layout::Size;
use ratatui::style::Style;
use ratatui::widgets::Block;
use tui_scrollview::ScrollViewState;

use crate::state::*;
use crate::ui::common::Component;
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::head::draw_head;
use crate::ui::render::pattern::draw_pattern_panel;
use crate::ui::render::sequence::draw_sequence_panel;
use crate::ui::render::style::{DEFAULT_BG, DEFAULT_FG};

pub fn draw_ui(
  frame: &mut Frame,
  state: &AppState,
  pattern_scroll_state: &mut ScrollViewState,
  optional_dialog: Option<&InputDialog<AppState>>,
) -> Size {
  // Set default colors for the entire frame
  frame.render_widget(
    Block::default().style(Style::default().bg(DEFAULT_BG).fg(DEFAULT_FG)),
    frame.area(),
  );

  let remaining_area = frame.area();
  let remaining_area = draw_head(frame, state, remaining_area);
  let remaining_area = draw_sequence_panel(frame, remaining_area, &state.sequence, &state.focus);
  let pattern_viewport_size =
    draw_pattern_panel(frame, state, pattern_scroll_state, remaining_area);

  if let Some(dialog) = optional_dialog {
    dialog.draw(frame);
  }

  pattern_viewport_size
}
