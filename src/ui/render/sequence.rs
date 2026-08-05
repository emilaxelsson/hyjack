use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect, Size};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::widgets::{Block, List, ListItem};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::state::*;
use crate::ui::common::rect_from_origin;
use crate::ui::render::style::{
  FOCUSED_ROW_BG, INACTIVE_FOCUSED_ROW_BG, PANEL_AREA_STYLE, SONG_END_STYLE, TITLE_STYLE,
};
use crate::ui::widgets::title_block::TitleBlock;

pub fn scroll_focused_sequence_into_view(
  state: &AppState,
  sequence_scroll_state: &mut ScrollViewState,
  viewport_height: u16,
) {
  let mut scroll_offset = sequence_scroll_state.offset();
  let viewport_height = viewport_height.max(1);
  let mut focused_row = state.focus.sequence_index as u16;
  let last_visible_row = scroll_offset
    .y
    .saturating_add(viewport_height.saturating_sub(1));

  if focused_row < scroll_offset.y {
    scroll_offset.y = focused_row;
  }

  if state.sequence.end_of_song <= focused_row as usize {
    focused_row += 1;
  }

  if focused_row >= last_visible_row {
    scroll_offset.y = focused_row
      .saturating_add(1)
      .saturating_sub(viewport_height);
  }

  sequence_scroll_state.set_offset(scroll_offset);
}

#[allow(clippy::too_many_arguments)]
fn draw_list_block(
  frame: &mut Frame,
  area: Rect,
  title: &str,
  items: &[String],
  focused_index: usize,
  end_index: usize,
  is_active: bool,
  sequence_scroll_state: &mut ScrollViewState,
) -> u16 {
  let block = TitleBlock::new()
    .title(title)
    .title_style(TITLE_STYLE)
    .style(PANEL_AREA_STYLE);
  let inner_area = block.inner(area);

  frame.render_widget(block, area);

  let mut list_items: Vec<ListItem> = Vec::new();
  items.iter().enumerate().for_each(|(idx, name)| {
    if idx == focused_index {
      let focused_row_bg = if is_active {
        FOCUSED_ROW_BG
      } else {
        INACTIVE_FOCUSED_ROW_BG
      };
      list_items.push(ListItem::new(name.clone()).bg(focused_row_bg));
    } else {
      list_items.push(ListItem::new(name.clone()));
    }
    if idx == end_index {
      list_items.push(ListItem::new("– End –").style(SONG_END_STYLE));
    }
  });

  let list = List::new(list_items).highlight_style(Style::default().add_modifier(Modifier::BOLD));

  // Add one to account for the "End" row
  let content_height = (items.len() as u16).saturating_add(1);
  let content_width = inner_area
    .width
    .saturating_sub(u16::from(content_height > inner_area.height))
    .max(1);
  let content_size = Size::new(content_width, content_height);
  let content_area = rect_from_origin(content_size);
  let mut scroll_view =
    ScrollView::new(content_size).scrollbars_visibility(ScrollbarVisibility::Automatic);

  scroll_view.render_widget(Block::default().style(PANEL_AREA_STYLE), content_area);
  scroll_view.render_widget(list, content_area);

  frame.render_stateful_widget(scroll_view, inner_area, sequence_scroll_state);

  inner_area.height.max(1)
}

pub fn draw_sequence_panel(
  frame: &mut Frame,
  area: Rect,
  sequence: &Sequence,
  focus: &Focus,
  is_active: bool,
  sequence_scroll_state: &mut ScrollViewState,
) -> (Rect, u16) {
  let width = sequence.patterns.iter().map(|s| s.len()).max().unwrap_or(0) as u16 + 1;
  let width = width.max(15) + 1;

  let [sequence_panel_area, _gap, remaining_area] = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Length(width),
      Constraint::Length(1),
      Constraint::Min(0),
    ])
    .areas(area);

  let viewport_height = draw_list_block(
    frame,
    sequence_panel_area,
    "Sequence",
    &sequence.patterns,
    focus.sequence_index,
    sequence.end_of_song,
    is_active,
    sequence_scroll_state,
  );

  (remaining_area, viewport_height)
}
