use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect, Size};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Cell, Row, Table};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::state::pattern::{Note, Pattern};
use crate::state::*;
use crate::ui::common::{get_absolute_position, rect_from_origin};
use crate::ui::render::style::{
  BEHIND_FOCUSED_ROW_STYLE, BEHIND_INACTIVE_FOCUSED_ROW_STYLE, TITLE_STYLE, default_block,
};
use crate::ui::render::track::{
  TRACK_COLUMN_WIDTH, draw_track_names, draw_tracks, track_column_offset, track_start_offset,
};
use crate::ui::widgets::title_block::TitleBlock;

/// Compute the logical position of the current focus in the pattern area.
fn pattern_focus_position(state: &AppState) -> Position {
  let cursor_x_offset =
    track_start_offset(state.focus.track_index) + track_column_offset(&state.focus.track_column);
  Position {
    x: cursor_x_offset as u16,
    y: state.focus.row_index as u16,
  }
}

pub fn scroll_cursor_into_view(
  state: &AppState,
  pattern_scroll_state: &mut ScrollViewState,
  viewport_size: Size,
) {
  let focus_position = pattern_focus_position(state);
  let mut scroll_offset = pattern_scroll_state.offset();
  let viewport_width = viewport_size.width.max(1);
  let viewport_height = viewport_size.height.max(1);
  let current_track_start = track_start_offset(state.focus.track_index) as u16;
  let current_track_end = current_track_start + TRACK_COLUMN_WIDTH.saturating_sub(1);
  let last_visible_row = scroll_offset
    .y
    .saturating_add(viewport_height.saturating_sub(1));
  let last_visible_column = scroll_offset
    .x
    .saturating_add(viewport_width.saturating_sub(1));

  if focus_position.y <= scroll_offset.y {
    scroll_offset.y = focus_position.y.saturating_sub(1);
  }

  if focus_position.y >= last_visible_row {
    scroll_offset.y = focus_position
      .y
      .saturating_sub(viewport_height.saturating_sub(2));
  }

  if current_track_end > last_visible_column {
    scroll_offset.x = current_track_end
      .saturating_add(1)
      .saturating_sub(viewport_width);
  }

  if current_track_start < scroll_offset.x {
    scroll_offset.x = current_track_start;
  }

  pattern_scroll_state.set_offset(scroll_offset);
}

/// Compute the effective visible viewport inside the pattern panel after accounting for
/// scrollbars that may appear when the content overflows in either dimension.
fn scrollable_viewport_size(inner_area: Rect, content_size: Size) -> Size {
  let mut viewport_width = inner_area.width;
  let mut viewport_height = inner_area.height;

  // Iterate until the scrollbar decision is stable, because adding a vertical scrollbar
  // reduces width and can force a horizontal scrollbar, and vice versa.
  loop {
    let has_vertical_scrollbar = content_size.height > viewport_height;
    let has_horizontal_scrollbar = content_size.width > viewport_width;
    let next_width = inner_area
      .width
      .saturating_sub(u16::from(has_vertical_scrollbar));
    let next_height = inner_area
      .height
      .saturating_sub(u16::from(has_horizontal_scrollbar));

    if next_width == viewport_width && next_height == viewport_height {
      return Size::new(viewport_width, viewport_height);
    }

    viewport_width = next_width;
    viewport_height = next_height;
  }
}

fn is_focus_visible(
  focus_position: Position,
  scroll_offset: Position,
  viewport_size: Size,
) -> bool {
  let viewport_width = viewport_size.width.max(1);
  let viewport_height = viewport_size.height.max(1);

  focus_position.x >= scroll_offset.x
    && focus_position.x < scroll_offset.x.saturating_add(viewport_width)
    && focus_position.y >= scroll_offset.y
    && focus_position.y < scroll_offset.y.saturating_add(viewport_height)
}

fn draw_pattern_details(
  frame: &mut Frame<'_>,
  pattern: &Pattern,
  note: Option<Note>,
  instr_name: &str,
  area: Rect,
) {
  let instr_ix = note
    .as_ref()
    .map(|n| n.instrument.to_string())
    .unwrap_or("-".to_string());
  let widths = vec![
    Constraint::Length(7),
    Constraint::Length(7),
    Constraint::Min(0),
  ];
  let rows = vec![
    Row::new(vec![
      Cell::from("Note").style(Style::default().bold()),
      Cell::from(format!("{}/{}", instr_ix, instr_name)),
    ]),
    Row::new(vec![
      Cell::from("Pattern").style(Style::default().bold()),
      Cell::from(format!("{} rows", pattern.num_rows.get())),
      Cell::from(format!("RPB {:2}", pattern.rows_per_beat)),
    ]),
  ];
  let table = Table::new(rows, widths).column_spacing(2);
  frame.render_widget(table, area);
}

pub fn draw_pattern_panel(
  frame: &mut Frame,
  state: &AppState,
  is_active: bool,
  pattern_scroll_state: &mut ScrollViewState,
  area: Rect,
) -> Size {
  let [pattern_area, details_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(0), Constraint::Length(2)])
    .areas(area);

  let current_pattern_title = state
    .sequence
    .titles
    .get(state.sequence.focus_index)
    .unwrap();
  let current_pattern = state
    .patterns
    .iter()
    .find(|p| p.title == *current_pattern_title)
    .unwrap();

  let panel_block = TitleBlock::new()
    .title(current_pattern_title.clone())
    .title_style(TITLE_STYLE);
  let inner_area = panel_block.inner(pattern_area);

  let [track_names_area, notes_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Length(1), Constraint::Min(0)])
    .areas(inner_area);

  frame.render_widget(panel_block, pattern_area);

  let num_tracks = current_pattern.tracks.len();
  let content_width = track_start_offset(num_tracks).saturating_sub(1) as u16;
  let content_height = current_pattern
    .tracks
    .iter()
    .map(|track| track.notes.len() as u16)
    .max()
    .unwrap_or(0);
  let content_size = Size::new(content_width, content_height);
  let viewport_size = scrollable_viewport_size(notes_area, content_size);
  let mut scroll_view =
    ScrollView::new(content_size).scrollbars_visibility(ScrollbarVisibility::Automatic);

  scroll_view.render_widget(default_block(), rect_from_origin(viewport_size));

  let focused_row_area = Rect {
    x: 0,
    y: state.focus.row_index as u16,
    width: content_width,
    height: 1,
  };
  let focused_row_style = if is_active {
    BEHIND_FOCUSED_ROW_STYLE
  } else {
    BEHIND_INACTIVE_FOCUSED_ROW_STYLE
  };
  scroll_view.render_widget(Block::default().style(focused_row_style), focused_row_area);

  draw_tracks(
    current_pattern,
    content_height,
    &mut scroll_view,
    focused_row_area,
    is_active,
  );

  frame.render_stateful_widget(scroll_view, notes_area, pattern_scroll_state);

  let scroll_offset = pattern_scroll_state.offset();
  draw_track_names(
    frame,
    track_names_area,
    scroll_offset.x,
    current_pattern,
    content_width,
  );

  let focus_position = pattern_focus_position(state);
  if is_active && is_focus_visible(focus_position, scroll_offset, viewport_size) {
    let cursor_pos = get_absolute_position(notes_area, scroll_offset, focus_position);
    frame.set_cursor_position(cursor_pos);
  }
  let current_note = current_pattern.get_note(state.focus.row_index, state.focus.track_index);
  let current_instrument = current_note
    .clone()
    .and_then(|note| state.instruments.get_name(note.instrument))
    .unwrap_or("-".to_string());

  draw_pattern_details(
    frame,
    current_pattern,
    current_note,
    &current_instrument,
    details_area,
  );

  viewport_size
}
