use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::widgets::{Block, Cell, List, ListItem, Paragraph, Row, Table};

use crate::state::*;
use crate::ui::common::{AdjustableArea, Component, DEFAULT_BG, DEFAULT_FG};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::widgets::horizontal_rule::HorizontalRule;
use crate::ui::widgets::title_block::TitleBlock;

const SUB_TITLE_BG: Color = Color::Rgb(60, 60, 60);
const TITLE_BG: Color = Color::Rgb(40, 80, 40);
const TITLE_STYLE: Style = Style::new()
  .bg(TITLE_BG)
  .fg(DEFAULT_FG)
  .add_modifier(Modifier::BOLD);
const SUB_TITLE_STYLE: Style = Style::new().bg(SUB_TITLE_BG).fg(DEFAULT_FG);
const PANEL_AREA_STYLE: Style = Style::new().bg(Color::Rgb(35, 35, 35)).fg(DEFAULT_FG);
const ACTIVE_ROW_BG: Color = Color::Rgb(55, 55, 150);
const ACTIVE_ROW_STYLE: Style = Style::new().bg(ACTIVE_ROW_BG);
const BEHIND_ACTIVE_ROW_STYLE: Style = Style::new().bg(Color::Rgb(45, 45, 120));

const TRACK_COLUMN_WIDTH: u16 = 15;

fn midi_pitch_to_tracker(pitch: u8) -> String {
  const NAMES: [&str; 12] = [
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
  ];
  let name = NAMES[(pitch as usize) % 12];
  let octave = (pitch / 12).saturating_sub(1);
  format!("{}{}", name, octave)
}

fn note_cell_text(note: &Option<Note>) -> String {
  if let Some(note) = note {
    format!(
      "{} {:02} v{:02}  ...",
      midi_pitch_to_tracker(note.pitch),
      note.instrument,
      note.velocity,
    )
  } else {
    "... ..  ..  ...".to_string()
  }
}

fn track_focus_offset(track_focus: &TrackFocus) -> usize {
  match track_focus {
    TrackFocus::Note => 0,
    TrackFocus::Instrument => 4,
    TrackFocus::Velocity => 8,
    TrackFocus::Effect => 12,
  }
}

fn draw_head(frame: &mut Frame, state: &AppState, area: Rect) -> Rect {
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

fn draw_sequence_panel(frame: &mut Frame, area: Rect, sequence: &[String], focus: &Focus) -> Rect {
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

  let [sequence_area, _gap, pool_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage(70),
      Constraint::Length(1),
      Constraint::Percentage(30),
    ])
    .areas(sequence_panel_area);

  draw_list_block(
    frame,
    sequence_area,
    "Sequence",
    sequence,
    focus.sequence_index,
  );
  draw_list_block(
    frame,
    pool_area,
    "Pool",
    &["Bridge B".to_string(), "Intro X".to_string()],
    0,
  );

  remaining_area
}

fn get_cursor_position(state: &AppState, active_row_area: Rect) -> Position {
  let cursor_x_offset = state.focus.pattern_track_index * (TRACK_COLUMN_WIDTH as usize + 1)
    + track_focus_offset(&state.focus.track_focus);
  Position {
    x: active_row_area.x + cursor_x_offset as u16,
    y: active_row_area.y,
  }
}

fn draw_track(
  frame: &mut Frame<'_>,
  inner_area: Rect,
  vertical_scroll_offset: usize,
  active_row_area: Rect,
  current_track_area: &Rect,
  track: &Track,
) {
  let active_row_track_area = active_row_area.intersection(*current_track_area);
  frame.render_widget(
    TitleBlock::default()
      .title(track.name.clone())
      .title_style(SUB_TITLE_STYLE)
      .style(PANEL_AREA_STYLE),
    *current_track_area,
  );
  frame.render_widget(
    Block::default().style(ACTIVE_ROW_STYLE),
    active_row_track_area,
  );

  for row_idx in vertical_scroll_offset..track.notes.len() {
    let note = &track.notes[row_idx];
    let cell_text = note_cell_text(note);
    let cell_area = Rect {
      x: current_track_area.x,
      y: inner_area.y + row_idx.saturating_sub(vertical_scroll_offset) as u16 + 1,
      width: TRACK_COLUMN_WIDTH,
      height: 1,
    };
    // Prevent rendering outside the inner area (causing panic)
    let cell_area = cell_area.intersection(inner_area);
    frame.render_widget(Paragraph::new(cell_text), cell_area);
  }
}

fn draw_pattern_details(frame: &mut Frame<'_>, current_pattern: &Pattern, area: Rect) {
  let widths = vec![Constraint::Length(7), Constraint::Min(0)];
  let rows = vec![
    Row::new(vec![
      Cell::from("Note").style(Style::default().bold()),
      Cell::from("2/Hihat"),
    ]),
    Row::new(vec![
      Cell::from("Pattern").style(Style::default().bold()),
      Cell::from(format!("{} beats", current_pattern.beats)),
      Cell::from(format!("RPB {:2}", current_pattern.rpb)),
    ]),
  ];
  let table = Table::new(rows, widths).column_spacing(2);
  frame.render_widget(table, area);
}

fn draw_pattern_panel(frame: &mut Frame, state: &AppState, area: Rect) {
  let [pattern_area, details_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Min(0), Constraint::Length(2)])
    .areas(area);

  let current_pattern_title = state.sequence.get(state.focus.sequence_index).unwrap();
  let current_pattern = state
    .patterns
    .iter()
    .find(|p| p.title == *current_pattern_title)
    .unwrap();

  let panel_block = TitleBlock::new()
    .title(current_pattern_title.clone())
    .title_style(TITLE_STYLE);
  let inner_area = panel_block.inner(pattern_area);
  let vertical_scroll_offset = state.focus.pattern_scroll_offset.rows;

  frame.render_widget(panel_block, pattern_area);

  let num_tracks = current_pattern.tracks.len();
  // Area for active row across all tracks
  let active_row_area = Rect {
    x: inner_area.x,
    y: inner_area.y
      + state
        .focus
        .pattern_row_index
        .saturating_sub(vertical_scroll_offset) as u16
      + 1,
    width: (num_tracks as u16 * (TRACK_COLUMN_WIDTH + 1)).saturating_sub(1),
    height: 1,
  };
  frame.render_widget(
    Block::default().style(BEHIND_ACTIVE_ROW_STYLE),
    active_row_area,
  );

  let mut current_track_area = inner_area;
  current_track_area.width = TRACK_COLUMN_WIDTH;
  for track in current_pattern.tracks.iter() {
    draw_track(
      frame,
      inner_area,
      vertical_scroll_offset,
      active_row_area,
      &current_track_area,
      track,
    );
    current_track_area = current_track_area.offset(Offset {
      x: TRACK_COLUMN_WIDTH as i32 + 1,
      y: 0,
    });
  }

  let cursor_pos = get_cursor_position(state, active_row_area);
  frame.set_cursor_position(cursor_pos);

  draw_pattern_details(frame, current_pattern, details_area);
}

pub fn draw_ui(
  frame: &mut Frame,
  state: &AppState,
  optional_dialog: Option<&InputDialog<AppState>>,
) {
  // Set default colors for the entire frame
  frame.render_widget(
    Block::default().style(Style::default().bg(DEFAULT_BG).fg(DEFAULT_FG)),
    frame.area(),
  );

  let remaining_area = frame.area();
  let remaining_area = draw_head(frame, state, remaining_area);
  let remaining_area = draw_sequence_panel(frame, remaining_area, &state.sequence, &state.focus);
  draw_pattern_panel(frame, state, remaining_area);

  if let Some(dialog) = optional_dialog {
    dialog.draw(frame);
  }
}
