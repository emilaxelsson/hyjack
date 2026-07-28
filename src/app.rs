use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tui_scrollview::ScrollViewState;

use crate::state::AppState;
use crate::ui::common::{Component, ComponentAction};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::draw_ui;

enum UIMode {
  EditPattern,
  EditSequence,
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
  pattern_scroll_state: &mut ScrollViewState,
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

  Ok(match mode {
    UIMode::EditPattern => match key.code {
      KeyCode::Char('q') | KeyCode::Esc => true,
      KeyCode::F(2) => {
        *optional_dialog = Some(pattern_title_dialog(state));
        false
      }
      KeyCode::Tab => {
        *mode = UIMode::EditSequence;
        false
      }
      KeyCode::Up if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_up();
        false
      }
      KeyCode::Up => {
        state.to_previous_row();
        false
      }
      KeyCode::Down if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_down();
        false
      }
      KeyCode::Down => {
        state.to_next_row();
        false
      }
      KeyCode::Left if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_left();
        false
      }
      KeyCode::Left => {
        state.to_previous_track_field();
        false
      }
      KeyCode::Right if key.modifiers == KeyModifiers::CONTROL => {
        pattern_scroll_state.scroll_right();
        false
      }
      KeyCode::Right => {
        state.to_next_track_field();
        false
      }
      _ => false,
    },
    UIMode::EditSequence => match key.code {
      KeyCode::Char('q') | KeyCode::Esc => true,
      KeyCode::Tab => {
        *mode = UIMode::EditPattern;
        false
      }
      KeyCode::Up => {
        state.to_previous_pattern();
        false
      }
      KeyCode::Down => {
        state.to_next_pattern();
        false
      }
      _ => false,
    },
  })
}

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
  let mut state = AppState::default();
  let mut mode = UIMode::EditPattern;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut pattern_scroll_state = ScrollViewState::default();

  loop {
    terminal.draw(|frame| draw_ui(frame, &state, &mut pattern_scroll_state, dialog.as_ref()))?;

    if event::poll(Duration::from_millis(100))? {
      let event = event::read()?;
      let quit = handle_event(
        &mut mode,
        event,
        &mut state,
        &mut pattern_scroll_state,
        &mut dialog,
      )?;
      if quit {
        break;
      }
    }
  }

  Ok(())
}
