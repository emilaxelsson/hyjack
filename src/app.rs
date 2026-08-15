use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Size;
use ratatui::style::Style;
use tui_scrollview::ScrollViewState;

use crate::state::AppState;
use crate::ui::common::{Component, ComponentAction, UIMode};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::pattern::scroll_cursor_into_view;
use crate::ui::render::style::{ERROR_BANNER_STYLE, INFO_BANNER_STYLE};
use crate::ui::render::{draw_ui, scroll_focused_sequence_into_view};

const SAVE_BANNER_TIMEOUT: Duration = Duration::from_millis(1400);

#[derive(Clone, Copy)]
struct Viewports {
  sequence_height: u16,
  pattern_size: Size,
}

struct Banner {
  text: String,
  style: Style,
  expires_at: Instant,
}

struct HandleEventResult {
  quit: bool,
  change_mode: Option<UIMode>,
  banner: Option<Banner>,
}

impl Banner {
  fn with_timeout(text: String, style: Style, timeout: Duration) -> Self {
    Self {
      text,
      style,
      expires_at: Instant::now() + timeout,
    }
  }

  fn is_expired(&self) -> bool {
    Instant::now() >= self.expires_at
  }
}

fn pattern_title_dialog(state: &AppState) -> InputDialog<AppState> {
  let current_title = state.get_current_pattern().title.clone();
  InputDialog {
    title: " Rename Pattern ".to_string(),
    label: "Pattern title".to_string(),
    cursor_index: current_title.chars().count(),
    input: current_title,
    error: None,
    on_submit: AppState::rename_focused_pattern,
  }
}

fn handle_edit_sequence_event(
  state: &mut AppState,
  sequence_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  optional_dialog: &mut Option<InputDialog<AppState>>,
  key: event::KeyEvent,
) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;
  let mut banner: Option<Banner> = None;

  match key.code {
    KeyCode::F(2) => {
      *optional_dialog = Some(pattern_title_dialog(state));
    }
    KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
      state.sequence.duplicate_focused_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Delete => {
      let delete_error = state.sequence.soft_delete_focused_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );

      if let Some(reason) = delete_error {
        banner = Some(Banner::with_timeout(
          reason,
          ERROR_BANNER_STYLE,
          SAVE_BANNER_TIMEOUT,
        ));
      }
    }
    KeyCode::Tab => {
      new_mode = Some(UIMode::EditPattern);
    }
    KeyCode::Up => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.sequence.move_title_up();
      } else {
        state.sequence.to_previous_title();
      }
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Down => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.sequence.move_title_down();
      } else {
        state.sequence.to_next_title();
      }
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    _ => {}
  }

  HandleEventResult {
    quit: false,
    change_mode: new_mode,
    banner,
  }
}

fn handle_edit_pattern_event(
  state: &mut AppState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  optional_dialog: &mut Option<InputDialog<AppState>>,
  key: event::KeyEvent,
) -> HandleEventResult {
  let bounds = state.get_current_pattern().bounds();
  let mut new_mode: Option<UIMode> = None;

  match key.code {
    KeyCode::F(2) => {
      *optional_dialog = Some(pattern_title_dialog(state));
    }
    KeyCode::Tab => {
      new_mode = Some(UIMode::EditSequence);
    }
    KeyCode::Up if key.modifiers == KeyModifiers::CONTROL => {
      pattern_scroll_state.scroll_up();
    }
    KeyCode::Up => {
      state.focus.to_previous_row();
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Down if key.modifiers == KeyModifiers::CONTROL => {
      pattern_scroll_state.scroll_down();
    }
    KeyCode::Down => {
      state.focus.to_next_row(bounds);
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Left if key.modifiers == KeyModifiers::CONTROL => {
      pattern_scroll_state.scroll_left();
    }
    KeyCode::Left => {
      state.focus.to_previous_track_field();
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Right if key.modifiers == KeyModifiers::CONTROL => {
      pattern_scroll_state.scroll_right();
    }
    KeyCode::Right => {
      state.focus.to_next_track_field(bounds);
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    _ => {}
  }

  HandleEventResult {
    quit: false,
    change_mode: new_mode,
    banner: None,
  }
}

fn handle_event(
  mode: &UIMode,
  event: Event,
  state: &mut AppState,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  optional_dialog: &mut Option<InputDialog<AppState>>,
) -> Result<HandleEventResult> {
  if let Some(dialog) = optional_dialog.as_mut() {
    if let ComponentAction::Close = dialog.handle_event(event, state)? {
      *optional_dialog = None;
    }
    return Ok(HandleEventResult {
      quit: false,
      change_mode: None,
      banner: None,
    });
  }

  let Event::Key(key) = event else {
    return Ok(HandleEventResult {
      quit: false,
      change_mode: None,
      banner: None,
    });
  };

  if matches!(key.code, KeyCode::Char('q')) {
    return Ok(HandleEventResult {
      quit: true,
      change_mode: None,
      banner: None,
    });
  }

  match mode {
    UIMode::EditPattern => {
      let result =
        handle_edit_pattern_event(state, pattern_scroll_state, viewports, optional_dialog, key);
      Ok(result)
    }
    UIMode::EditSequence => {
      let result = handle_edit_sequence_event(
        state,
        sequence_scroll_state,
        viewports,
        optional_dialog,
        key,
      );
      Ok(result)
    }
  }
}

pub fn run_app(
  terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
  song_file_path: &Path,
) -> Result<()> {
  let mut state = AppState::load_from_file(song_file_path)?;
  let mut mode = UIMode::EditPattern;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut banner: Option<Banner> = None;
  let mut sequence_scroll_state = ScrollViewState::default();
  let mut pattern_scroll_state = ScrollViewState::default();
  let mut viewports = Viewports {
    sequence_height: 1,
    pattern_size: Size::new(1, 1),
  };

  loop {
    if banner.as_ref().is_some_and(Banner::is_expired) {
      banner = None;
    }

    terminal.draw(|frame| {
      (viewports.sequence_height, viewports.pattern_size) = draw_ui(
        frame,
        &state,
        mode,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        banner
          .as_ref()
          .map(|banner| (banner.text.as_str(), banner.style)),
        dialog.as_ref(),
      );
    })?;

    if event::poll(Duration::from_millis(100))? {
      let event = event::read()?;

      if let Event::Key(key) = &event
        && key.code == KeyCode::Char('s')
        && key.modifiers == KeyModifiers::CONTROL
      {
        banner = Some(match state.save_to_file(song_file_path) {
          Ok(()) => Banner::with_timeout(
            format!("Saved {}", song_file_path.display()),
            INFO_BANNER_STYLE,
            SAVE_BANNER_TIMEOUT,
          ),
          Err(err) => Banner::with_timeout(
            format!("Failure saving {}: {}", song_file_path.display(), err),
            ERROR_BANNER_STYLE,
            SAVE_BANNER_TIMEOUT,
          ),
        });
        continue;
      }

      let result = handle_event(
        &mode,
        event,
        &mut state,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
        &mut dialog,
      )?;

      if result.quit {
        break;
      }

      if let Some(new_mode) = result.change_mode {
        mode = new_mode;
      }

      if let Some(new_banner) = result.banner {
        banner = Some(new_banner);
      }

      if mode == UIMode::EditPattern && result.change_mode.is_some() {
        state.refocus_pattern();
      }
    }
  }

  Ok(())
}
