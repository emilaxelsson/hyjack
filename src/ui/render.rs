mod head;
pub mod pattern;
mod sequence;
pub mod style;
mod track;

use ratatui::Frame;
use ratatui::layout::Size;
use ratatui::style::Style;
use tui_scrollview::ScrollViewState;

pub use self::sequence::scroll_focused_sequence_into_view;
use crate::player::Player;
use crate::state::*;
use crate::ui::common::{Component, UIMode};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::head::draw_head;
use crate::ui::render::pattern::draw_pattern_panel;
use crate::ui::render::sequence::draw_sequence_panel;
use crate::ui::render::style::default_block;
use crate::ui::widgets::banner::Banner;

pub struct DrawState<'a> {
  pub app: &'a AppState,
  pub player: &'a Player,
  pub mode: &'a UIMode,
}

pub fn draw_ui(
  frame: &mut Frame,
  draw_state: DrawState,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  optional_banner: Option<(&str, Style)>,
  optional_dialog: Option<&InputDialog<AppState>>,
) -> (u16, Size) {
  let DrawState { app, player, mode } = draw_state;

  // Set default colors for the entire frame
  frame.render_widget(default_block(), frame.area());

  let remaining_area = frame.area();
  let remaining_area = draw_head(frame, app, player, remaining_area);
  let sequence_is_active = matches!(mode, UIMode::EditSequence);
  let pattern_is_active = matches!(mode, UIMode::EditPattern | UIMode::EditPatternElement);
  let (remaining_area, sequence_viewport_height) = draw_sequence_panel(
    frame,
    remaining_area,
    &app.sequence,
    sequence_is_active,
    sequence_scroll_state,
  );
  let pattern_viewport_size = draw_pattern_panel(
    frame,
    mode,
    app,
    pattern_is_active,
    pattern_scroll_state,
    remaining_area,
  );

  if let Some(dialog) = optional_dialog {
    dialog.draw(frame);
  }

  if let Some((text, style)) = optional_banner {
    frame.render_widget(Banner::new(text).style(style), frame.area());
  }

  (sequence_viewport_height, pattern_viewport_size)
}
