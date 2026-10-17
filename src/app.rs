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
use crate::ui::render::{DrawState, draw_ui, scroll_focused_sequence_into_view};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ViewportSync {
  #[default]
  None,
  Pattern,
  Sequence,
  PlayerPosition,
}

struct HandleEventResult {
  /// Whether the event was handled by the UI. Intended for testing.
  handled: bool,
  quit: bool,
  new_mode: Option<UIMode>,
  banner: Option<Banner>,
  dialog: Option<InputDialog<AppState>>,
  viewport_sync: ViewportSync,
}

struct EventContext<'a> {
  pattern_scroll_state: &'a mut ScrollViewState,
  viewports: Viewports,
  optional_dialog: &'a mut Option<InputDialog<AppState>>,
  song_file_path: &'a Path,
  player: &'a mut Player,
  midi: &'a mut dyn MidiSink,
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
  viewports: Viewports,
  key: event::KeyEvent,
) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;
  let mut banner: Option<Banner> = None;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut viewport_sync = ViewportSync::None;
  let mut handled = false;

  match key.code {
    KeyCode::Up => {
      handled = true;
      if key.modifiers == KeyModifiers::CONTROL {
        state.sequence.move_title_up();
      } else {
        state.sequence.to_previous_title();
      }
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Down => {
      handled = true;
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
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::PageUp => {
      handled = true;
      state
        .sequence
        .to_previous_page(usize::from(viewports.sequence_height));
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::PageDown => {
      handled = true;
      state
        .sequence
        .to_next_page(usize::from(viewports.sequence_height));
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Home => {
      handled = true;
      state.sequence.to_first_title();
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::End => {
      handled = true;
      state.sequence.to_last_title();
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Char('d') if key.modifiers == KeyModifiers::NONE => {
      handled = true;
      state.sequence.duplicate_focused_title();
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
      handled = true;
      state.duplicate_focused_pattern();
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Insert => {
      handled = true;
      state.insert_new_pattern();
      viewport_sync = ViewportSync::Sequence;
    }
    KeyCode::Delete => {
      handled = true;
      let remove_error = state.sequence.soft_remove_focused_title();
      viewport_sync = ViewportSync::Sequence;

      if let Some(reason) = remove_error {
        banner = Some(Banner::with_timeout(
          reason,
          ERROR_BANNER_STYLE,
          SAVE_BANNER_TIMEOUT,
        ));
      }
    }
    KeyCode::F(2) => {
      handled = true;
      dialog = Some(pattern_title_dialog(state));
    }
    KeyCode::F(4) | KeyCode::Enter => {
      handled = true;
      new_mode = Some(UIMode::EditPattern);
    }
    _ => {}
  }

  HandleEventResult {
    handled,
    quit: false,
    new_mode,
    banner,
    dialog,
    viewport_sync,
  }
}

fn handle_edit_pattern_event(
  state: &mut AppState,
  pattern_scroll_state: &mut ScrollViewState,
  player: &Player,
  midi: &mut dyn MidiSink,
  key: event::KeyEvent,
) -> Result<HandleEventResult> {
  let mut new_mode: Option<UIMode> = None;
  let mut dialog: Option<InputDialog<AppState>> = None;
  let mut viewport_sync = ViewportSync::None;
  let mut handled = false;

  match key.code {
    KeyCode::Up => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_up();
      } else {
        state.to_previous_pattern_row();
        viewport_sync = ViewportSync::Pattern;
      }
      handled = true;
    }
    KeyCode::Down => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_down();
      } else {
        state.to_next_pattern_row();
        viewport_sync = ViewportSync::Pattern;
      }
      handled = true;
    }
    KeyCode::PageUp => {
      state.page_up_pattern();
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::PageDown => {
      state.page_down_pattern();
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::Home => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.to_first_song_row();
      } else {
        state.to_first_pattern_column();
      }
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::End => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.to_last_song_or_pool_row();
      } else {
        state.to_last_pattern_column();
      }
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
      state.to_previous_pattern_track();
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::Tab => {
      state.to_next_pattern_track();
      viewport_sync = ViewportSync::Pattern;
      handled = true;
    }
    KeyCode::Left => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_left();
      } else {
        state.to_previous_pattern_field();
        viewport_sync = ViewportSync::Pattern;
      }
      handled = true;
    }
    KeyCode::Right => {
      if key.modifiers == KeyModifiers::CONTROL {
        pattern_scroll_state.scroll_right();
      } else {
        state.to_next_pattern_field();
        viewport_sync = ViewportSync::Pattern;
      }
      handled = true;
    }
    KeyCode::F(2) => {
      dialog = Some(pattern_title_dialog(state));
      handled = true;
    }
    KeyCode::F(3) => {
      new_mode = Some(UIMode::EditSequence);
      handled = true;
    }
    KeyCode::Delete => {
      state.remove_focused_note();
      handled = true;
    }
    KeyCode::Enter => {
      new_mode = Some(UIMode::EditPatternElement);
      handled = true;
    }
    KeyCode::Char('z') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Octave));
      handled = true;
    }
    KeyCode::Char('x') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Velocity));
      handled = true;
    }
    KeyCode::Char('c') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Step));
      handled = true;
    }
    KeyCode::Char('v') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Instrument));
      handled = true;
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Instrument
        && ch.is_ascii_digit()
        && can_type_note(key.modifiers) =>
    {
      if let Some(digit) = ch.to_digit(10) {
        state.set_focused_note_instrument_digit(digit as u8, Instant::now());
        handled = true;
      }
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Velocity
        && ch.is_ascii_digit()
        && can_type_note(key.modifiers) =>
    {
      if let Some(digit) = ch.to_digit(10) {
        state.set_focused_note_velocity_digit(digit as u8, Instant::now());
        handled = true;
      }
    }
    KeyCode::Char(ch) if can_type_note(key.modifiers) => {
      if let Some(semitone) = note_key_to_semitone(ch) {
        let note = state.editor_settings.make_note(semitone);
        player.emit_note_on(&note, midi)?;
        state.set_focused_note(note);
        state.step_rows();
        handled = true;
      }
    }
    _ => {}
  }

  Ok(HandleEventResult {
    handled,
    quit: false,
    new_mode,
    banner: None,
    dialog,
    viewport_sync,
  })
}

#[cfg(test)]
mod tests {
  use ratatui::backend::TestBackend;

  use super::*;
  use crate::player::{MIDI_NOTE_ON, MIDI_PROGRAM_CHANGE, SilentMidiSink};
  use crate::state::minimal_song;
  use crate::state::pattern::{Note, TrackColumn};

  struct RecordingMidiSink(Vec<Vec<u8>>);

  impl MidiSink for RecordingMidiSink {
    fn send(&mut self, message: &[u8]) -> Result<()> {
      self.0.push(message.to_vec());
      Ok(())
    }
  }

  fn test_state() -> AppState {
    serde_json::from_str(include_str!("../songs/test.json")).unwrap()
  }

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
    let mut state = test_state();
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

  #[test]
  fn navigation_handlers_request_their_viewport_to_sync() {
    let mut state = test_state();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = SilentMidiSink;
    let viewports = Viewports {
      sequence_height: 1,
      pattern_size: Size::new(1, 1),
    };

    let result = handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Down),
    )
    .unwrap();
    assert_eq!(result.viewport_sync, ViewportSync::Pattern);

    let result = handle_edit_sequence_event(&mut state, viewports, KeyEvent::from(KeyCode::Down));
    assert_eq!(result.viewport_sync, ViewportSync::Sequence);
  }

  #[test]
  fn minimal_song_navigation_smoke_test() {
    let mut state = minimal_song();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = SilentMidiSink;
    let viewports = Viewports {
      sequence_height: 1,
      pattern_size: Size::new(1, 1),
    };

    for key in [
      KeyEvent::from(KeyCode::Up),
      KeyEvent::from(KeyCode::Down),
      KeyEvent::from(KeyCode::Left),
      KeyEvent::from(KeyCode::Right),
      KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL),
      KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
      KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
      KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::Tab),
      KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT),
      KeyEvent::from(KeyCode::PageUp),
      KeyEvent::from(KeyCode::PageDown),
      KeyEvent::from(KeyCode::Home),
      KeyEvent::from(KeyCode::End),
      KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL),
      KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::F(2)),
      KeyEvent::from(KeyCode::F(3)),
      KeyEvent::from(KeyCode::Char('z')),
      KeyEvent::from(KeyCode::Char('x')),
      KeyEvent::from(KeyCode::Char('c')),
      KeyEvent::from(KeyCode::Char('v')),
    ] {
      let result = handle_edit_pattern_event(
        &mut state,
        &mut pattern_scroll_state,
        &player,
        &mut midi,
        key,
      )
      .unwrap();
      assert!(result.handled, "pattern key {key:?} was not handled");
    }

    let result = handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Char('?')),
    )
    .unwrap();
    assert!(!result.handled);

    for key in [
      KeyEvent::from(KeyCode::Up),
      KeyEvent::from(KeyCode::Down),
      KeyEvent::from(KeyCode::PageUp),
      KeyEvent::from(KeyCode::PageDown),
      KeyEvent::from(KeyCode::Home),
      KeyEvent::from(KeyCode::End),
      KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL),
      KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::F(2)),
      KeyEvent::from(KeyCode::Char('d')),
      KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::Insert),
      KeyEvent::from(KeyCode::Delete),
      KeyEvent::from(KeyCode::F(4)),
      KeyEvent::from(KeyCode::Enter),
    ] {
      let result = handle_edit_sequence_event(&mut state, viewports, key);
      assert!(result.handled, "sequence key {key:?} was not handled");
    }
  }

  #[test]
  fn minimal_song_rendering_smoke_test() {
    let state = minimal_song();
    let mut sequence_scroll_state = ScrollViewState::default();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
      .draw(|frame| {
        draw_ui(
          frame,
          DrawState {
            app: &state,
            player: &player,
            mode: &UIMode::EditPattern,
          },
          &mut sequence_scroll_state,
          &mut pattern_scroll_state,
          None,
          None,
        );
      })
      .unwrap();
  }

  #[test]
  fn horizontal_navigation_enters_virtual_tracks() {
    let mut state = test_state();
    let stored_tracks = state.get_current_pattern().tracks.len();
    state.track_titles.push("Virtual".to_owned());
    state.focus.track_index = stored_tracks - 1;
    state.focus.track_column = TrackColumn::Note;
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = SilentMidiSink;

    handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Tab),
    )
    .unwrap();

    assert_eq!(state.focus.track_index, stored_tracks);
    assert_eq!(state.focus.track_column, TrackColumn::Note);

    state.focus.track_index = stored_tracks - 1;
    state.focus.track_column = TrackColumn::Effect;
    handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Right),
    )
    .unwrap();

    assert_eq!(state.focus.track_index, stored_tracks);
    assert_eq!(state.focus.track_column, TrackColumn::Note);
  }

  #[test]
  fn delete_removes_the_focused_note() {
    let mut state = test_state();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = SilentMidiSink;
    let note = state.editor_settings.make_note(Semitone(0));
    state.set_focused_note(note);

    handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Delete),
    )
    .unwrap();

    let pattern = state.get_current_pattern();
    assert!(
      pattern
        .get_note(state.focus.row_index, state.focus.track_index)
        .is_none()
    );
  }

  #[test]
  fn inserting_a_note_emits_its_midi_note_on() {
    let mut state = test_state();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    let row_index = state.focus.row_index;
    let track_index = state.focus.track_index;
    let note = state.editor_settings.make_note(Semitone(0));

    handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Char('a')),
    )
    .unwrap();

    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, note.instrument.0],
        vec![MIDI_NOTE_ON, 48, note.velocity.0],
      ]
    );
    let inserted_note = state
      .get_current_pattern()
      .get_note(row_index, track_index)
      .unwrap();
    assert_eq!(inserted_note.octave, note.octave);
    assert_eq!(inserted_note.semitone, note.semitone);
    assert_eq!(inserted_note.instrument, note.instrument);
    assert_eq!(inserted_note.velocity, note.velocity);
  }

  #[test]
  fn inserting_a_note_works_from_any_column() {
    for track_column in [
      TrackColumn::Note,
      TrackColumn::Instrument,
      TrackColumn::Velocity,
      TrackColumn::Effect,
    ] {
      let mut state = test_state();
      state.focus.track_column = track_column;
      let row_index = state.focus.row_index;
      let track_index = state.focus.track_index;

      handle_edit_pattern_event(
        &mut state,
        &mut ScrollViewState::default(),
        &Player::default(),
        &mut SilentMidiSink,
        KeyEvent::from(KeyCode::Char('a')),
      )
      .unwrap();

      assert!(
        state
          .get_current_pattern()
          .get_note(row_index, track_index)
          .is_some(),
        "note was not inserted with {track_column:?} focused"
      );
    }
  }

  #[test]
  fn dialog_and_player_modes_capture_global_shortcuts() {
    let mut state = test_state();
    let mut pattern_scroll_state = ScrollViewState::default();
    let viewports = Viewports {
      sequence_height: 1,
      pattern_size: Size::new(1, 1),
    };
    let mut dialog = Some(pattern_title_dialog(&state));
    let mut player = Player::default();
    let mut midi = SilentMidiSink;
    let quit = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));

    let result = {
      let mut context = EventContext {
        pattern_scroll_state: &mut pattern_scroll_state,
        viewports,
        optional_dialog: &mut dialog,
        song_file_path: Path::new("test.json"),
        player: &mut player,
        midi: &mut midi,
      };
      handle_event(&UIMode::EditPattern, quit.clone(), &mut state, &mut context).unwrap()
    };
    assert!(!result.quit);

    dialog = None;
    let result = {
      let mut context = EventContext {
        pattern_scroll_state: &mut pattern_scroll_state,
        viewports,
        optional_dialog: &mut dialog,
        song_file_path: Path::new("test.json"),
        player: &mut player,
        midi: &mut midi,
      };
      handle_event(
        &UIMode::Player {
          previous_mode: Box::new(UIMode::EditPattern),
        },
        quit,
        &mut state,
        &mut context,
      )
      .unwrap()
    };
    assert!(!result.quit);
  }
}

fn handle_edit_pattern_element_event(state: &mut AppState, key: KeyEvent) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;
  let mut handled = false;

  match key.code {
    KeyCode::Up => {
      state.adjust_focused_note(true, true);
      handled = true;
    }
    KeyCode::Down => {
      state.adjust_focused_note(false, true);
      handled = true;
    }
    KeyCode::PageUp => {
      state.adjust_focused_note(true, false);
      handled = true;
    }
    KeyCode::PageDown => {
      state.adjust_focused_note(false, false);
      handled = true;
    }
    KeyCode::Home => {
      state.set_focused_note_to_bound(false);
      handled = true;
    }
    KeyCode::End => {
      state.set_focused_note_to_bound(true);
      handled = true;
    }
    KeyCode::Enter | KeyCode::Esc => {
      new_mode = Some(UIMode::EditPattern);
      handled = true;
    }
    _ => {}
  }

  HandleEventResult {
    handled,
    quit: false,
    new_mode,
    banner: None,
    dialog: None,
    viewport_sync: ViewportSync::None,
  }
}

fn handle_edit_settings_event(
  state: &mut AppState,
  key: KeyEvent,
  current_setting: EditorSettingFocus,
) -> HandleEventResult {
  let mut new_mode: Option<UIMode> = None;
  let mut handled = false;

  let max_instrument = state.instruments.max_index().unwrap_or(InstrumentIndex(0));

  match key.code {
    KeyCode::Up => {
      state
        .editor_settings
        .increase(current_setting, true, max_instrument);
      handled = true;
    }
    KeyCode::Down => {
      state
        .editor_settings
        .decrease(current_setting, true, max_instrument);
      handled = true;
    }
    KeyCode::PageUp => {
      state
        .editor_settings
        .increase(current_setting, false, max_instrument);
      handled = true;
    }
    KeyCode::PageDown => {
      state
        .editor_settings
        .decrease(current_setting, false, max_instrument);
      handled = true;
    }
    KeyCode::Home => {
      state.editor_settings.set_to_minimum(current_setting);
      handled = true;
    }
    KeyCode::End => {
      state
        .editor_settings
        .set_to_maximum(current_setting, max_instrument);
      handled = true;
    }
    KeyCode::Char('z') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Octave));
      handled = true;
    }
    KeyCode::Char('x') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Velocity));
      handled = true;
    }
    KeyCode::Char('c') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Step));
      handled = true;
    }
    KeyCode::Char('v') => {
      new_mode = Some(UIMode::EditSetting(EditorSettingFocus::Instrument));
      handled = true;
    }
    KeyCode::Left => {
      new_mode = Some(UIMode::EditSetting(current_setting.previous()));
      handled = true;
    }
    KeyCode::Right => {
      new_mode = Some(UIMode::EditSetting(current_setting.next()));
      handled = true;
    }
    KeyCode::Enter | KeyCode::Esc => {
      new_mode = Some(UIMode::EditPattern);
      handled = true;
    }
    _ => {}
  }

  HandleEventResult {
    handled,
    quit: false,
    new_mode,
    banner: None,
    dialog: None,
    viewport_sync: ViewportSync::None,
  }
}

fn handle_event(
  mode: &UIMode,
  event: Event,
  state: &mut AppState,
  context: &mut EventContext<'_>,
) -> Result<HandleEventResult> {
  let handled = matches!(event, Event::Key(_));

  if let Some(dialog) = context.optional_dialog.as_mut() {
    if let ComponentAction::Close = dialog.handle_event(event, state)? {
      *context.optional_dialog = None;
    }
    return Ok(HandleEventResult {
      handled,
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::None,
    });
  }

  let Event::Key(key) = event else {
    return Ok(HandleEventResult {
      handled: false,
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::None,
    });
  };

  if key.code == KeyCode::Char(' ') && key.modifiers == KeyModifiers::NONE {
    if let UIMode::Player { previous_mode } = mode {
      context.player.stop(context.midi)?;
      return Ok(HandleEventResult {
        handled: true,
        quit: false,
        new_mode: Some((**previous_mode).clone()),
        banner: None,
        dialog: None,
        viewport_sync: ViewportSync::None,
      });
    }

    context.player.start(state, context.midi, Instant::now())?;
    return Ok(HandleEventResult {
      handled: true,
      quit: false,
      new_mode: Some(UIMode::Player {
        previous_mode: Box::new(mode.clone()),
      }),
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::PlayerPosition,
    });
  }

  if matches!(mode, UIMode::Player { .. }) {
    return Ok(HandleEventResult {
      handled: false,
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::None,
    });
  }

  if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL {
    let banner = match state.save_to_file(context.song_file_path) {
      Ok(()) => Banner::with_timeout(
        format!("Saved {}", context.song_file_path.display()),
        INFO_BANNER_STYLE,
        SAVE_BANNER_TIMEOUT,
      ),
      Err(err) => Banner::with_timeout(
        format!(
          "Failure saving {}: {}",
          context.song_file_path.display(),
          err
        ),
        ERROR_BANNER_STYLE,
        SAVE_BANNER_TIMEOUT,
      ),
    };
    return Ok(HandleEventResult {
      handled: true,
      quit: false,
      new_mode: None,
      banner: Some(banner),
      dialog: None,
      viewport_sync: ViewportSync::None,
    });
  }

  if key.code == KeyCode::Char('q') && key.modifiers == KeyModifiers::CONTROL {
    return Ok(HandleEventResult {
      handled: true,
      quit: true,
      new_mode: None,
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::None,
    });
  }

  match mode {
    UIMode::EditPattern => handle_edit_pattern_event(
      state,
      context.pattern_scroll_state,
      context.player,
      context.midi,
      key,
    ),
    UIMode::EditPatternElement => Ok(handle_edit_pattern_element_event(state, key)),
    UIMode::EditSequence => {
      let result = handle_edit_sequence_event(state, context.viewports, key);
      Ok(result)
    }
    UIMode::EditSetting(setting) => {
      let result = handle_edit_settings_event(state, key, *setting);
      Ok(result)
    }
    UIMode::Player { .. } => Ok(HandleEventResult {
      handled: false,
      quit: false,
      new_mode: None,
      banner: None,
      dialog: None,
      viewport_sync: ViewportSync::None,
    }),
  }
}

/// Align editor focus and scrolling with the current playback position.
fn sync_player_position(
  state: &mut AppState,
  player: &Player,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
) {
  state.sequence.focus_index = player.state().sequence_index;
  state.focus.row_index = player.state().row_index;
  state.refocus_pattern();
  scroll_focused_sequence_into_view(
    &state.sequence,
    sequence_scroll_state,
    viewports.sequence_height,
  );
  scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
}

fn sync_viewport(
  viewport_sync: ViewportSync,
  state: &mut AppState,
  player: &Player,
  sequence_scroll_state: &mut ScrollViewState,
  pattern_scroll_state: &mut ScrollViewState,
  viewports: Viewports,
) {
  match viewport_sync {
    ViewportSync::None => {}
    ViewportSync::Pattern => {
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
    }
    ViewportSync::Sequence => {
      scroll_focused_sequence_into_view(
        &state.sequence,
        sequence_scroll_state,
        viewports.sequence_height,
      );
    }
    ViewportSync::PlayerPosition => {
      sync_player_position(
        state,
        player,
        sequence_scroll_state,
        pattern_scroll_state,
        viewports,
      );
    }
  }
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
        DrawState {
          app: &state,
          player: &player,
          mode: &mode,
        },
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

      let result = {
        let mut context = EventContext {
          pattern_scroll_state: &mut pattern_scroll_state,
          viewports,
          optional_dialog: &mut dialog,
          song_file_path,
          player: &mut player,
          midi,
        };
        handle_event(&mode, event, &mut state, &mut context)?
      };

      if result.quit {
        if matches!(mode, UIMode::Player { .. }) {
          player.stop(midi)?;
        }
        break;
      }

      let viewport_sync = if result.handled {
        result.viewport_sync
      } else {
        ViewportSync::None
      };
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

      sync_viewport(
        viewport_sync,
        &mut state,
        &player,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
      );
    } else if matches!(mode, UIMode::Player { .. }) {
      player.tick(&state, midi, Instant::now())?;
      sync_player_position(
        &mut state,
        &player,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
      );
    }
  }

  Ok(())
}
