use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::state::*;
use crate::ui::common::AdjustableArea;
use crate::ui::widgets::horizontal_rule::HorizontalRule;

pub fn draw_head(frame: &mut Frame, state: &AppState, area: Rect) -> Rect {
  let top_line = format!(
    "Dracker {}    {:02}:{:02}:{:02}.{:03} | {} - {}",
    state.dracker_version,
    state.player.time.hour,
    state.player.time.minute,
    state.player.time.second,
    state.player.time.millis,
    state.song_title,
    state.sequence.get(state.player.sequence_index).unwrap(),
  );
  let editing_line = format!(
    "Editing        Octave {} | Velocity {} | Step {:2} | Instrument  {}",
    3, 50, 4, "3/Kick",
  );

  frame.render_widget(Paragraph::new(top_line), area);
  let area = area.squeeze_down(1);
  frame.render_widget(HorizontalRule, area);
  let area = area.squeeze_down(1);
  frame.render_widget(Paragraph::new(editing_line), area);
  let area = area.squeeze_down(1);
  frame.render_widget(HorizontalRule, area);

  area.squeeze_down(1)
}
