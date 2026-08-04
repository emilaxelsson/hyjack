use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::widgets::{List, ListItem};

use crate::state::*;
use crate::ui::render::style::{ACTIVE_ROW_BG, PANEL_AREA_STYLE, TITLE_STYLE};
use crate::ui::widgets::title_block::TitleBlock;

fn draw_list_block(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  items: &[String],
  active_index: usize,
) {
  let block = TitleBlock::new()
    .title(title)
    .title_style(TITLE_STYLE)
    .style(PANEL_AREA_STYLE);
  let inner_area = block.inner(area);

  frame.render_widget(block, area);

  let list_items: Vec<ListItem> = items
    .iter()
    .enumerate()
    .map(|(idx, name)| {
      if idx == active_index {
        ListItem::new(name.clone()).bg(ACTIVE_ROW_BG)
      } else {
        ListItem::new(name.clone())
      }
    })
    .collect();

  let list = List::new(list_items).highlight_style(Style::default().add_modifier(Modifier::BOLD));

  frame.render_widget(list, inner_area);
}

pub fn draw_sequence_panel(
  frame: &mut Frame,
  area: Rect,
  sequence: &[String],
  focus: &Focus,
) -> Rect {
  let width = sequence.iter().map(|s| s.len()).max().unwrap_or(0) as u16 + 1;
  let width = width.max(15) + 1;

  let [sequence_panel_area, _gap, remaining_area] = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length(width),
      Constraint::Length(1),
      Constraint::Min(0),
    ])
    .areas(area);

  draw_list_block(
    frame,
    sequence_panel_area,
    "Sequence",
    sequence,
    focus.sequence_index,
  );

  remaining_area
}
