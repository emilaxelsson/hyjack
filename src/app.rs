use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Size;
use ratatui::style::Style;
use tui_scrollview::ScrollViewState;

use crate::player::{MidiSink, Player};
use crate::state::AppState;
use crate::state::editor_settings::EditorSettingFocus;
use crate::state::instruments::InstrumentIndex;
use crate::state::pattern::{Semitone, TrackColumn};
use crate::ui::common::{Component, ComponentAction, UIMode};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::pattern::scroll_cursor_into_view;
use crate::ui::render::style::{ERROR_BANNER_STYLE, INFO_BANNER_STYLE};
use crate::ui::render::{draw_ui, scroll_focused_sequence_into_view};

const SAVE_BANNER_TIMEOUT: Duration = Duration::from_millis(1400);
const MAX_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);

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
  new_mode: Option<UIMode>,
  banner: Option<Banner>,
  dialog: Option<InputDialog<AppState>>,
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

fn note_key_to_semitone(ch: char) -> Option<Semitone> {
  match ch.to_ascii_lowercase() {
    'a' => Some(Semitone(0)),
    'w' => Some(Semitone(1)),
    's' => Some(Semitone(2)),
    'e' => Some(Semitone(3)),
    'd' => Some(Semitone(4)),
    'f' => Some(Semitone(5)),
    't' => Some(Semitone(6)),
    'g' => Some(Semitone(7)),
    'y' => Some(Semitone(8)),
    'h' => Some(Semitone(9)),
    'u' => Some(Semitone(10)),
    'j' => Some(Semitone(11)),
    _ => None,
  }
}

fn can_type_note(modifiers: KeyModifiers) -> bool {
  !modifiers.contains(KeyModifiers::CONTROL)
    && !modifiers.contains(KeyModifiers::ALT)
    && !modifiers.contains(KeyModifiers::SUPER)
}

fn handle_edit_sequence_event(
  state: &mut AppState,
  sequence_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  key: event::KeyEvent,
) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;
  let mut banner: Option<Banner> = None;
  let mut dialog: Option<InputDialog<AppState>> = None;

  match key.code {
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
        if let Some(reason) = state.sequence.move_title_down() {
          banner = Some(Banner::with_timeout(
            reason,
            ERROR_BANNER_STYLE,
            SAVE_BANNER_TIMEOUT,
          ));
        }
      } else {
        state.sequence.to_next_title();
      }
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::PageUp => {
      state
        .sequence
        .to_previous_page(usize::from(viewports.sequence_height));
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::PageDown => {
      state
        .sequence
        .to_next_page(usize::from(viewports.sequence_height));
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Home => {
      state.sequence.to_first_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::End => {
      state.sequence.to_last_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
      state.sequence.duplicate_focused_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
      state.duplicate_focused_pattern();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Insert => {
      state.insert_new_pattern();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    KeyCode::Delete => {
      let remove_error = state.sequence.soft_remove_focused_title();
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );

      if let Some(reason) = remove_error {
        banner = Some(Banner::with_timeout(
          reason,
          ERROR_BANNER_STYLE,
          SAVE_BANNER_TIMEOUT,
        ));
      }
    }
    KeyCode::F(2) => {
      dialog = Some(pattern_title_dialog(state));
    }
    KeyCode::F(4) | KeyCode::Enter => {
      new_mode = Some(UIMode::EditPattern);
    }
    _ => {}
  }

  HandleEventResult {
    quit: false,
    new_mode,
    banner,
    dialog,
  }
}

fn handle_edit_pattern_event(
  state: &mut AppState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
  key: event::KeyEvent,
) -> HandleEventResult {
  let bounds = state.get_current_pattern().bounds();
  let mut new_mode: Option<UIMode> = None;
  let mut dialog: Option<InputDialog<AppState>> = None;

  match key.code {
    KeyCode::Up => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_up();
      } else {
        state.to_previous_pattern_row();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
    }
    KeyCode::Down => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_down();
      } else {
        state.to_next_pattern_row();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
    }
    KeyCode::PageUp => {
      state.page_up_pattern();
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::PageDown => {
      state.page_down_pattern();
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Home => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.to_first_song_row();
      } else {
        state.to_first_pattern_column();
      }
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::End => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.to_last_song_or_pool_row();
      } else {
        state.to_last_pattern_column();
      }
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
      state.focus.to_previous_track();
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Tab => {
      state.focus.to_next_track(bounds);
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    KeyCode::Left => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_left();
      } else {
        state.focus.to_previous_track_field();
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
    }
    KeyCode::Right => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_right();
      } else {
        state.focus.to_next_track_field(bounds);
        scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
      }
    }
    KeyCode::F(2) => {
      dialog = Some(pattern_title_dialog(state));
    }
    KeyCode::F(3) => {
      new_mode = Some(UIMode::EditSequence);
    }
    KeyCode::Enter => {
      new_mode = Some(UIMode::EditPatternElement);
    }
    KeyCode::Char('z') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Octave));
    }
    KeyCode::Char('x') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Velocity));
    }
    KeyCode::Char('c') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Step));
    }
    KeyCode::Char('v') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Instrument));
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Instrument && can_type_note(key.modifiers) =>
    {
      if let Some(digit) = ch.to_digit(10) {
        state.set_focused_note_instrument_digit(digit as u8, Instant::now());
      }
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Velocity && can_type_note(key.modifiers) =>
    {
      if let Some(digit) = ch.to_digit(10) {
        state.set_focused_note_velocity_digit(digit as u8, Instant::now());
      }
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Note && can_type_note(key.modifiers) =>
    {
      if let Some(semitone) = note_key_to_semitone(ch) {
        let note = state.editor_settings.make_note(semitone);
        state.set_focused_note(note);
        state.step_rows();
      }
    }
    _ => {}
  }

  HandleEventResult {
    quit: false,
    new_mode,
    banner: None,
    dialog,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::pattern::Note;

  #[test]
  fn note_key_to_semitone_maps_requested_layout() {
    assert_eq!(note_key_to_semitone('a'), Some(Semitone(0)));
    assert_eq!(note_key_to_semitone('w'), Some(Semitone(1)));
    assert_eq!(note_key_to_semitone('s'), Some(Semitone(2)));
    assert_eq!(note_key_to_semitone('e'), Some(Semitone(3)));
    assert_eq!(note_key_to_semitone('d'), Some(Semitone(4)));
    assert_eq!(note_key_to_semitone('f'), Some(Semitone(5)));
    assert_eq!(note_key_to_semitone('t'), Some(Semitone(6)));
    assert_eq!(note_key_to_semitone('g'), Some(Semitone(7)));
    assert_eq!(note_key_to_semitone('y'), Some(Semitone(8)));
    assert_eq!(note_key_to_semitone('h'), Some(Semitone(9)));
    assert_eq!(note_key_to_semitone('u'), Some(Semitone(10)));
    assert_eq!(note_key_to_semitone('j'), Some(Semitone(11)));
  }

  #[test]
  fn editor_setting_home_end_set_each_setting_to_its_bound() {
    let mut state: AppState = serde_json::from_str(include_str!("../songs/test.json")).unwrap();
    let max_instrument = state.instruments.max_index().unwrap();

    for setting in [
      EditorSettingFocus::Octave,
      EditorSettingFocus::Velocity,
      EditorSettingFocus::Step,
      EditorSettingFocus::Instrument,
    ] {
      handle_edit_settings_event(&mut state, KeyEvent::from(KeyCode::End), setting);
      match setting {
        EditorSettingFocus::Octave => assert_eq!(state.editor_settings.octave.0, Note::MAX_OCTAVE),
        EditorSettingFocus::Velocity => assert_eq!(state.editor_settings.velocity.0, 99),
        EditorSettingFocus::Step => assert_eq!(state.editor_settings.step, 16),
        EditorSettingFocus::Instrument => {
          assert_eq!(state.editor_settings.instrument, max_instrument);
        }
      }

      handle_edit_settings_event(&mut state, KeyEvent::from(KeyCode::Home), setting);
      match setting {
        EditorSettingFocus::Octave => assert_eq!(state.editor_settings.octave.0, 0),
        EditorSettingFocus::Velocity => assert_eq!(state.editor_settings.velocity.0, 0),
        EditorSettingFocus::Step => assert_eq!(state.editor_settings.step, 0),
        EditorSettingFocus::Instrument => {
          assert_eq!(state.editor_settings.instrument, InstrumentIndex(0));
        }
      }
    }
  }
}

fn handle_edit_pattern_element_event(state: &mut AppState, key: KeyEvent) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;

  match key.code {
    KeyCode::Up => state.adjust_focused_note(true, true),
    KeyCode::Down => state.adjust_focused_note(false, true),
    KeyCode::PageUp => state.adjust_focused_note(true, false),
    KeyCode::PageDown => state.adjust_focused_note(false, false),
    KeyCode::Home => state.set_focused_note_to_bound(false),
    KeyCode::End => state.set_focused_note_to_bound(true),
    KeyCode::Enter | KeyCode::Esc => new_mode = Some(UIMode::EditPattern),
    _ => {}
  }

  HandleEventResult {
    quit: false,
    new_mode,
    banner: None,
    dialog: None,
  }
}

fn handle_edit_settings_event(
  state: &mut AppState,
  key: KeyEvent,
  current_setting: EditorSettingFocus,
) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;

  let max_instrument = state.instruments.max_index().unwrap_or(InstrumentIndex(0));

  match key.code {
    KeyCode::Up => {
      state
        .editor_settings
        .increase(current_setting, true, max_instrument);
    }
    KeyCode::Down => {
      state
        .editor_settings
        .decrease(current_setting, true, max_instrument);
    }
    KeyCode::PageUp => {
      state
        .editor_settings
        .increase(current_setting, false, max_instrument);
    }
    KeyCode::PageDown => {
      state
        .editor_settings
        .decrease(current_setting, false, max_instrument);
    }
    KeyCode::Home => {
      state.editor_settings.set_to_minimum(current_setting);
    }
    KeyCode::End => {
      state
        .editor_settings
        .set_to_maximum(current_setting, max_instrument);
    }
    KeyCode::Char('z') => new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Octave)),
    KeyCode::Char('x') => new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Velocity)),
    KeyCode::Char('c') => new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Step)),
    KeyCode::Char('v') => new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Instrument)),
    KeyCode::Left => new_mode = Some(UIMode::EditSetting(current_setting.previous())),
    KeyCode::Right => new_mode = Some(UIMode::EditSetting(current_setting.next())),
    KeyCode::Enter | KeyCode::Esc => {
      new_mode = Some(UIMode::EditPattern);
    }
    _ => {}
  }

  HandleEventResult {
    quit: false,
    new_mode,
    banner: None,
    dialog: None,
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
      new_mode: None,
      banner: None,
      dialog: None,
    });
  }

  let Event::Key(key) = event else {
    return Ok(HandleEventResult {
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
    });
  };

  if matches!(key.code, KeyCode::Char('q')) && key.modifiers == KeyModifiers::CONTROL {
    return Ok(HandleEventResult {
      quit: true,
      new_mode: None,
      banner: None,
      dialog: None,
    });
  }

  match mode {
    UIMode::EditPattern => {
      let result = handle_edit_pattern_event(state, pattern_scroll_state, viewports, key);
      Ok(result)
    }
    UIMode::EditPatternElement => Ok(handle_edit_pattern_element_event(state, key)),
    UIMode::EditSequence => {
      let result = handle_edit_sequence_event(state, sequence_scroll_state, viewports, key);
      Ok(result)
    }
    UIMode::EditSetting(setting) => {
      let result = handle_edit_settings_event(state, key, *setting);
      Ok(result)
    }
    UIMode::Player { .. } => Ok(HandleEventResult {
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
    }),
  }
}

/// Align editor focus and scrolling with the current playback position.
fn sync_player_position(
  state: &mut AppState,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
) {
  state.sequence.focus_index = state.player.sequence_index;
  state.focus.row_index = state.player.row_index;
  state.refocus_pattern();
  scroll_focused_sequence_into_view(
    &state.sequence,
    sequence_scroll_state,
    viewports.sequence_height,
  );
  scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
}

pub fn run_app(
  terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
  song_file_path: &Path,
  midi: &mut dyn MidiSink,
) -> Result<()> {
  let mut state = AppState::load_from_file(song_file_path)?;
  let mut mode = UIMode::EditPattern;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut banner: Option<Banner> = None;
  let mut player = Player::default();
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
        &mode,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        banner
          .as_ref()
          .map(|banner| (banner.text.as_str(), banner.style)),
        dialog.as_ref(),
      );
    })?;

    let timeout = match &mode {
      UIMode::Player { .. } => player
        .next_deadline()
        .expect("Player mode requires active playback")
        .saturating_duration_since(Instant::now())
        .min(MAX_EVENT_POLL_TIMEOUT),
      _ => MAX_EVENT_POLL_TIMEOUT,
    };

    if event::poll(timeout)? {
      let event = event::read()?;

      if let Event::Key(key) = &event
        && key.code == KeyCode::Char(' ')
        && key.modifiers == KeyModifiers::NONE
      {
        if matches!(mode, UIMode::Player { .. }) {
          let UIMode::Player { previous_mode } = mode else {
            unreachable!();
          };
          player.stop(midi)?;
          mode = *previous_mode;
        } else {
          dialog = None;
          player.start(&mut state, midi, Instant::now())?;
          mode = UIMode::Player {
            previous_mode: Box::new(mode),
          };
          sync_player_position(
            &mut state,
            &mut sequence_scroll_state,
            &mut pattern_scroll_state,
            viewports,
          );
        }
        continue;
      }

      if matches!(mode, UIMode::Player { .. }) {
        continue;
      }

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
        if matches!(mode, UIMode::Player { .. }) {
          player.stop(midi)?;
        }
        break;
      }

      let mode_changed = result.new_mode.is_some();
      if let Some(new_mode) = result.new_mode {
        mode = new_mode;
      }

      if let Some(new_banner) = result.banner {
        banner = Some(new_banner);
      }

      if let Some(new_dialog) = result.dialog {
        dialog = Some(new_dialog);
      }

      if matches!(mode, UIMode::EditPattern) && mode_changed {
        state.refocus_pattern();
      }
    } else if matches!(mode, UIMode::Player { .. }) {
      player.tick(&mut state, midi, Instant::now())?;
      sync_player_position(
        &mut state,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
      );
    }
  }

  Ok(())
}
