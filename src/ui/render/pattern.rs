use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect, Size};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Cell, Row, Table};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::state::instruments::InstrumentIndex;
use crate::state::pattern::{Note, Pattern};
use crate::state::*;
use crate::ui::common::{EditableField, UIMode, get_absolute_position, rect_from_origin};
use crate::ui::render::style::{
  BEHIND_FOCUSED_ROW_STYLE, BEHIND_INACTIVE_FOCUSED_ROW_STYLE, FOCUSED_ROW_BG, SUB_TITLE_BG,
  TITLE_STYLE, default_block,
};
use crate::ui::render::track::{
  TRACK_COLUMN_WIDTH, draw_track_names, draw_tracks, track_column_offset, track_start_offset,
};
use crate::ui::widgets::title_block::TitleBlock;

struct NoteInfo {
  note: Option<Note>,
  instrument_name: String,
}

struct EditingFields {
  octave: u8,
  velocity: u8,
  step: u8,
  instrument: InstrumentIndex,
  instrument_name: String,
}

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
  note: NoteInfo,
  pattern: &Pattern,
  editing_fields: EditingFields,
  focused_field: Option<EditableField>,
  area: Rect,
) {
  let instr_ix = note
    .note
    .as_ref()
    .map(|n| n.instrument.to_string())
    .unwrap_or("-".to_string());
  let editing_instr = format!(
    "{:2}/{}",
    editing_fields.instrument, editing_fields.instrument_name
  );

  let mut rows_bg = SUB_TITLE_BG;
  let mut rpb_bg = SUB_TITLE_BG;
  let mut octave_bg = SUB_TITLE_BG;
  let mut velocity_bg = SUB_TITLE_BG;
  let mut step_bg = SUB_TITLE_BG;
  let mut instr_bg = SUB_TITLE_BG;

  if let Some(field) = focused_field {
    match field {
      EditableField::NumRows => rows_bg = FOCUSED_ROW_BG,
      EditableField::RowsPerBeat => rpb_bg = FOCUSED_ROW_BG,
      EditableField::Octave => octave_bg = FOCUSED_ROW_BG,
      EditableField::Velocity => velocity_bg = FOCUSED_ROW_BG,
      EditableField::Step => step_bg = FOCUSED_ROW_BG,
      EditableField::Instrument => instr_bg = FOCUSED_ROW_BG,
    }
  }

  let widths = vec![
    Constraint::Length(7),
    Constraint::Length(11),
    Constraint::Length(13),
    Constraint::Length(9),
    Constraint::Length(8 + editing_instr.len() as u16),
  ];
  let rows = vec![
    Row::new(vec![
      Cell::from("Note").style(Style::default().bold()),
      Cell::from(format!("{:2}/{}", instr_ix, note.instrument_name)).column_span(4),
    ]),
    Row::new(vec![
      Cell::from("Pattern").style(Style::default().bold()),
      Cell::from(format!("(rows   {:2})", pattern.num_rows.get())).bg(rows_bg),
      Cell::from(format!("(rpb      {:2})", pattern.rows_per_beat)).bg(rpb_bg),
    ]),
    Row::new(vec![
      Cell::from("Editing").style(Style::default().bold()),
      Cell::from(format!("(octave {:2})", editing_fields.octave)).bg(octave_bg),
      Cell::from(format!("(velocity {})", editing_fields.velocity)).bg(velocity_bg),
      Cell::from(format!("(step {:2})", editing_fields.step)).bg(step_bg),
      Cell::from(format!("(instr {})", editing_instr)).bg(instr_bg),
    ]),
  ];
  let table = Table::new(rows, widths).column_spacing(1);
  frame.render_widget(table, area);

  let cursor_pos = match focused_field {
    Some(EditableField::NumRows) => Position {
      x: 17,
      y: 1,
    },
    Some(EditableField::RowsPerBeat) => Position {
      x: 31,
      y: 1,
    },
    Some(EditableField::Octave) => Position {
      x: 17,
      y: 2,
    },
    Some(EditableField::Velocity) => Position {
      x: 31,
      y: 2,
    },
    Some(EditableField::Step) => Position {
      x: 41,
      y: 2,
    },
    Some(EditableField::Instrument) => Position {
      x: 52,
      y: 2,
    },
    None => return,
  };
  let cursor_pos = Position {
    x: area.x.saturating_add(cursor_pos.x),
    y: area.y.saturating_add(cursor_pos.y),
  };
  frame.set_cursor_position(cursor_pos);
}

pub fn draw_pattern_panel(
  frame: &mut Frame,
  mode: UIMode,
  state: &AppState,
  is_active: bool,
  pattern_scroll_state: &mut ScrollViewState,
  area: Rect,
) -> Size {
  let [pattern_area, details_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(0), Constraint::Length(3)])
    .areas(area);

  let current_pattern_title = state
    .sequence
    .titles
    .get(state.sequence.focus_index)
    .expect("Sequence focus index is out of bounds");
  let current_pattern = state
    .patterns
    .iter()
    .find(|p| p.title == *current_pattern_title)
    .unwrap_or_else(|| panic!("Pattern not found: {}", current_pattern_title));

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
  let note_info = NoteInfo {
    note: current_note,
    instrument_name: current_instrument,
  };
  let editing_fields = EditingFields {
    octave: 3,
    velocity: 50,
    step: 4,
    instrument: 2,
    instrument_name: "Kick".to_string(),
  };
  let focused_field = match mode {
    UIMode::EditField(field) => Some(field),
    _ => None,
  };

  draw_pattern_details(
    frame,
    note_info,
    current_pattern,
    editing_fields,
    focused_field,
    details_area,
  );

  viewport_size
}
