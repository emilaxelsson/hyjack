use nonempty::NonEmpty;
use ratatui::Frame;
use ratatui::layout::{Offset, Position, Rect, Size};
use ratatui::widgets::{Block, Paragraph};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};

use crate::state::pattern::{Note, Octave, Pattern, Semitone, Track, TrackColumn};
use crate::ui::common::rect_from_origin;
use crate::ui::render::style::{
  ELEMENT_EDITING_FOCUSED_ROW_STYLE, FOCUSED_ROW_STYLE, INACTIVE_FOCUSED_ROW_STYLE,
  PANEL_AREA_STYLE, SUB_TITLE_STYLE, default_block,
};

pub const TRACK_COLUMN_WIDTH: u16 = 15;

fn tracker_note_text(octave: Octave, semitone: Semitone) -> String {
  const NAMES: [&str; 12] = [
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
  ];
  let name = NAMES[(semitone.0 as usize) % 12];
  format!("{}{}", name, octave.0)
}

fn note_cell_text(note: &Option<Note>) -> String {
  if let Some(note) = note {
    format!(
      "{} {:02} v{:02}  ...",
      tracker_note_text(note.octave, note.semitone),
      note.instrument.0,
      note.velocity.0,
    )
  } else {
    "... ..  ..  ...".to_string()
  }
}

/// Compute the x offset of a track column within a track.
pub fn track_column_offset(column: &TrackColumn) -> usize {
  match column {
    TrackColumn::Note => 0,
    TrackColumn::Instrument => 4,
    TrackColumn::Velocity => 8,
    TrackColumn::Effect => 12,
  }
}

/// Compute the starting x offset of a track based on its index in the pattern.
pub fn track_start_offset(track_index: usize) -> usize {
  track_index * (TRACK_COLUMN_WIDTH as usize + 1)
}

#[derive(Clone, Copy)]
pub enum TrackActivity {
  /// The pattern panel is inactive.
  Inactive,
  /// The pattern panel is active.
  Active,
  /// An element is being edited in this track.
  ElementEditing,
}

fn draw_track(
  scroll_view: &mut ScrollView,
  focused_row_area: Rect,
  content_height: u16,
  activity: TrackActivity,
  track_index: usize,
  track: Option<&Track>,
  num_rows: usize,
) {
  let current_track_area = Rect {
    x: track_start_offset(track_index) as u16,
    y: 0,
    width: TRACK_COLUMN_WIDTH,
    height: content_height,
  };
  let focused_row_track_area = focused_row_area.intersection(current_track_area);
  scroll_view.render_widget(Block::default().style(PANEL_AREA_STYLE), current_track_area);
  let focused_row_style = match activity {
    TrackActivity::Inactive => INACTIVE_FOCUSED_ROW_STYLE,
    TrackActivity::Active => FOCUSED_ROW_STYLE,
    TrackActivity::ElementEditing => ELEMENT_EDITING_FOCUSED_ROW_STYLE,
  };
  scroll_view.render_widget(
    Block::default().style(focused_row_style),
    focused_row_track_area,
  );

  for row_idx in 0..num_rows {
    let note = track
      .and_then(|track| track.notes.get(row_idx))
      .unwrap_or(&None);
    let cell_text = note_cell_text(note);
    let cell_area = Rect {
      x: current_track_area.x,
      y: row_idx as u16,
      width: TRACK_COLUMN_WIDTH,
      height: 1,
    };
    scroll_view.render_widget(Paragraph::new(cell_text), cell_area);
  }
}

pub fn draw_tracks(
  current_pattern: &Pattern,
  track_titles: &NonEmpty<String>,
  content_height: u16,
  scroll_view: &mut ScrollView,
  focused_row_area: Rect,
  activity: TrackActivity,
  editing_track_index: Option<usize>,
) {
  for track_index in 0..track_titles.len() {
    let track_activity = if editing_track_index == Some(track_index) {
      TrackActivity::ElementEditing
    } else {
      activity
    };
    draw_track(
      scroll_view,
      focused_row_area,
      content_height,
      track_activity,
      track_index,
      current_pattern.tracks.get(track_index),
      current_pattern.num_rows.get(),
    );
  }
}

pub fn draw_track_names(
  frame: &mut Frame,
  tracks_area: Rect,
  horizontal_scroll_offset: u16,
  track_titles: &NonEmpty<String>,
  content_width: u16,
) {
  let mut scroll_view =
    ScrollView::new(Size::new(content_width, 1)).scrollbars_visibility(ScrollbarVisibility::Never);

  scroll_view.render_widget(default_block(), rect_from_origin(tracks_area.as_size()));

  let distance = TRACK_COLUMN_WIDTH as i32 + 1;
  let offset = Offset { x: distance, y: 0 };
  let mut current_track_area = Rect {
    x: 0,
    y: 0,
    width: TRACK_COLUMN_WIDTH,
    height: 1,
  };
  for title in track_titles {
    scroll_view.render_widget(
      Paragraph::new(title.clone()).style(SUB_TITLE_STYLE),
      current_track_area,
    );
    current_track_area = current_track_area.offset(offset);
  }

  let scroll_offset = Position {
    x: horizontal_scroll_offset,
    y: 0,
  };
  let mut scroll_state = ScrollViewState::with_offset(scroll_offset);
  frame.render_stateful_widget(scroll_view, tracks_area, &mut scroll_state);
}
