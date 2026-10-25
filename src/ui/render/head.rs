use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::player::Player;
use crate::state::*;
use crate::ui::common::AdjustableArea;
use crate::ui::widgets::horizontal_rule::HorizontalRule;

pub fn draw_head(frame: &mut Frame, state: &AppState, player: &Player, area: Rect) -> Rect {
  let player_state = player.state();
  let time = player_state.time;
  let player_line = format!(
    "Hyjack {}     {:02}:{:02}:{:02}.{:03} | {} - {}",
    state.hyjack_version,
    time.as_secs() / 3_600,
    (time.as_secs() / 60) % 60,
    time.as_secs() % 60,
    time.subsec_millis(),
    state.song_title,
    state
      .sequence
      .titles
      .get(player_state.sequence_index)
      .unwrap(),
  );

  frame.render_widget(Paragraph::new(player_line), area);
  let area = area.squeeze_down(1);
  frame.render_widget(HorizontalRule, area);

  area.squeeze_down(1)
}
