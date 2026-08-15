use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::state::*;
use crate::ui::common::AdjustableArea;
use crate::ui::widgets::horizontal_rule::HorizontalRule;

pub fn draw_head(frame: &mut Frame, state: &AppState, area: Rect) -> Rect {
  let player_line = format!(
    "Dracker {}    {:02}:{:02}:{:02}.{:03} | {} - {}",
    state.dracker_version,
    state.player.time.hour,
    state.player.time.minute,
    state.player.time.second,
    state.player.time.millis,
    state.song_title,
    state
      .sequence
      .titles
      .get(state.player.sequence_index)
      .unwrap(),
  );

  frame.render_widget(Paragraph::new(player_line), area);
  let area = area.squeeze_down(1);
  frame.render_widget(HorizontalRule, area);

  area.squeeze_down(1)
}
