use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Size;
use tui_scrollview::ScrollViewState;

use crate::state::AppState;
use crate::ui::common::{Component, ComponentAction, UIMode};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::pattern::scroll_cursor_into_view;
use crate::ui::render::{draw_ui, scroll_focused_sequence_into_view};

#[derive(Clone, Copy)]
struct Viewports {
  sequence_height: u16,
  pattern_size: Size,
}

fn pattern_title_dialog(state: &AppState) -> InputDialog<AppState> {
  let current_title = state.get_current_pattern().title.clone();
  InputDialog {
    title: " Rename Pattern ".to_string(),
    label: "Pattern title".to_string(),
    cursor_index: current_title.chars().count(),
    input: current_title,
    error: None,
    on_submit: AppState::rename_current_pattern,
  }
}

fn handle_event(
  mode: &mut UIMode,
  event: Event,
  state: &mut AppState,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  optional_dialog: &mut Option<InputDialog<AppState>>,
) -> Result<bool> {
  if let Some(dialog) = optional_dialog.as_mut() {
    if let ComponentAction::Close = dialog.handle_event(event, state)? {
      *optional_dialog = None;
    }
    return Ok(false);
  }

  let Event::Key(key) = event else {
    return Ok(false);
  };

  if matches!(key.code, KeyCode::Char('q')) {
    return Ok(true);
  }

  match mode {
    UIMode::EditPattern => match key.code {
      KeyCode::F(2) => {
        *optional_dialog = Some(pattern_title_dialog(state));
      }
      KeyCode::Tab => {
        *mode = UIMode::EditSequence;
      }
      KeyCode::Up if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_up();
      }
      KeyCode::Up => {
        state.to_previous_row();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
      KeyCode::Down if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_down();
      }
      KeyCode::Down => {
        state.to_next_row();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
      KeyCode::Left if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_left();
      }
      KeyCode::Left => {
        state.to_previous_track_field();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
      KeyCode::Right if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_right();
      }
      KeyCode::Right => {
        state.to_next_track_field();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
      _ => {}
    },
    UIMode::EditSequence => match key.code {
      KeyCode::Tab => {
        *mode = UIMode::EditPattern;
      }
      KeyCode::Up => {
        state.to_previous_pattern();
        scroll_focused_sequence_into_view(state, sequence_scroll_state, viewports.sequence_height);
      }
      KeyCode::Down => {
        state.to_next_pattern();
        scroll_focused_sequence_into_view(state, sequence_scroll_state, viewports.sequence_height);
      }
      _ => {}
    },
  }

  Ok(false)
}

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
  let mut state = AppState::default();
  let mut mode = UIMode::EditPattern;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut sequence_scroll_state = ScrollViewState::default();
  let mut pattern_scroll_state = ScrollViewState::default();
  let mut viewports = Viewports {
    sequence_height: 1,
    pattern_size: Size::new(1, 1),
  };

  loop {
    terminal.draw(|frame| {
      (viewports.sequence_height, viewports.pattern_size) = draw_ui(
        frame,
        &state,
        mode,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        dialog.as_ref(),
      );
    })?;

    if event::poll(Duration::from_millis(100))? {
      let event = event::read()?;
      let quit = handle_event(
        &mut mode,
        event,
        &mut state,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
        &mut dialog,
      )?;
      if quit {
        break;
      }
    }
  }

  Ok(())
}
