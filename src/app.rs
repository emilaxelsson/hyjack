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
use crate::state::pattern::{Effect, Semitone, TrackColumn};
use crate::ui::common::{Component, ComponentAction, UIMode};
use crate::ui::component::input_dialog::InputDialog;
use crate::ui::render::pattern::scroll_cursor_into_view;
use crate::ui::render::style::{ERROR_BANNER_STYLE, INFO_BANNER_STYLE};
use crate::ui::render::{DrawState, draw_ui, scroll_focused_sequence_into_view};
use crate::utils::TwoDigits;

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
  PatternAndSequence,
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
    KeyCode::Insert if key.modifiers == KeyModifiers::CONTROL => {
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
    KeyCode::Insert if key.modifiers == KeyModifiers::CONTROL => {
      state.insert_new_pattern();
      viewport_sync = ViewportSync::PatternAndSequence;
      handled = true;
    }
    KeyCode::Delete => {
      let has_focused_effect = state.focus.track_column == TrackColumn::Effect
        && state
          .get_current_pattern()
          .get_note(state.focus.row_index, state.focus.track_index)
          .is_some_and(|note| note.effect.is_some());
      if has_focused_effect {
        state.set_focused_note_effect(None);
      } else {
        state.remove_focused_note();
      }
      handled = true;
    }
    KeyCode::Enter => {
      if key.modifiers == KeyModifiers::CONTROL {
        state.copy_focused_note_attributes_to_editor_settings();
      } else {
        new_mode = Some(UIMode::EditPatternElement);
      }
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
      if state.focus.track_column == TrackColumn::Effect
        && matches!(ch.to_ascii_lowercase(), 't' | 'd' | 'e')
        && can_type_note(key.modifiers)
        && state
          .get_current_pattern()
          .get_note(state.focus.row_index, state.focus.track_index)
          .is_some() =>
    {
      let value = TwoDigits::new(0).unwrap();
      let effect = match ch.to_ascii_lowercase() {
        't' => Effect::SetTempo(value),
        'd' => Effect::Delay(value),
        'e' => Effect::Advance(value),
        _ => unreachable!(),
      };
      state.set_focused_note_effect(Some(effect));
      handled = true;
    }
    KeyCode::Char(ch)
      if state.focus.track_column == TrackColumn::Effect
        && ch.is_ascii_digit()
        && can_type_note(key.modifiers) =>
    {
      if let Some(digit) = ch.to_digit(10) {
        state.set_focused_note_effect_digit(digit as u8, Instant::now());
        handled = true;
      }
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
        let mut note = state.editor_settings.make_note(semitone);
        note.velocity = state
          .instruments
          .snap_velocity(note.instrument, note.velocity);
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
  use crate::state::pattern::{Effect, Note, Octave, Track, TrackColumn, Velocity};

  struct RecordingMidiSink(Vec<Vec<u8>>);

  impl MidiSink for RecordingMidiSink {
    fn send(&mut self, message: &[u8]) -> Result<()> {
      self.0.push(message.to_vec());
      Ok(())
    }
  }

  fn test_state() -> AppState {
    serde_json::from_str(include_str!("../tests/test.json")).unwrap()
  }

  #[test]
  fn playback_deadlines_are_processed_during_continuous_input() {
    let mut state = minimal_song();
    let mut note = crate::state::test_state().patterns[0].tracks[0].notes[0]
      .clone()
      .unwrap();
    note.effect = Some(Effect::Delay(TwoDigits::new(90).unwrap()));
    state.patterns[0].tracks.push(Track {
      notes: vec![Some(note)],
    });
    let start = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, start).unwrap();
    let mode = UIMode::Player {
      previous_mode: Box::new(UIMode::EditPattern),
    };

    for (before, after, expected_timeout, expected_tick) in [
      (0, 60_000, 100_000, false),
      (60_000, 112_499, 52_500, false),
      (112_499, 112_500, 1, true),
      (112_500, 125_000, 12_500, true),
    ] {
      let mut times = [
        start + Duration::from_micros(before),
        start + Duration::from_micros(after),
      ]
      .into_iter();
      let (input_ready, ticked) = poll_and_process_playback(
        &mode,
        &state,
        &mut player,
        &mut midi,
        || times.next().unwrap(),
        |timeout| {
          assert_eq!(timeout, Duration::from_micros(expected_timeout));
          Ok(true)
        },
      )
      .unwrap();
      assert!(input_ready);
      assert_eq!(ticked, expected_tick);
    }

    assert_eq!(
      midi.0,
      vec![vec![MIDI_PROGRAM_CHANGE, 0], vec![MIDI_NOTE_ON, 36, 64]]
    );
    assert_eq!(player.state().time, Duration::ZERO);
  }

  #[test]
  fn poll_timeout_processes_playback_and_edit_mode_does_not_tick() {
    let state = crate::state::test_state();
    let start = Instant::now();
    let mut player = Player::default();
    let mut midi = RecordingMidiSink(Vec::new());
    player.start(&state, &mut midi, start).unwrap();
    let mode = UIMode::Player {
      previous_mode: Box::new(UIMode::EditPattern),
    };
    let deadline = player.next_deadline().unwrap();
    let mut times = [start, deadline].into_iter();
    let result = poll_and_process_playback(
      &mode,
      &state,
      &mut player,
      &mut midi,
      || times.next().unwrap(),
      |_| Ok(false),
    )
    .unwrap();
    assert_eq!(result, (false, true));

    let messages = midi.0.clone();
    let result = poll_and_process_playback(
      &UIMode::EditPattern,
      &state,
      &mut player,
      &mut midi,
      || panic!("edit mode does not need the clock"),
      |timeout| {
        assert_eq!(timeout, MAX_EVENT_POLL_TIMEOUT);
        Ok(true)
      },
    )
    .unwrap();
    assert_eq!(result, (true, false));
    assert_eq!(midi.0, messages);
  }

  #[test]
  fn test_song_loads_from_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test.json");
    let state = AppState::load_from_file(&path).unwrap();

    assert_eq!(state.sequence.end_of_song, 7);
    assert_eq!(state.get_current_pattern().title, "Intro B");
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
  fn insert_creates_pattern_from_pattern_panel() {
    let mut state = test_state();
    let initial_pattern_count = state.patterns.len();
    let mut pattern_scroll_state = ScrollViewState::default();
    let result = handle_edit_pattern_event(
      &mut state,
      &mut pattern_scroll_state,
      &Player::default(),
      &mut SilentMidiSink,
      KeyEvent::new(KeyCode::Insert, KeyModifiers::CONTROL),
    )
    .unwrap();

    assert!(result.handled);
    assert_eq!(result.viewport_sync, ViewportSync::PatternAndSequence);
    assert_eq!(state.patterns.len(), initial_pattern_count + 1);
    assert_eq!(state.get_current_pattern().title, "New pattern");
  }

  #[test]
  fn pattern_panel_smoke_test() {
    let mut state = minimal_song();
    let mut pattern_scroll_state = ScrollViewState::default();
    let player = Player::default();
    let mut midi = SilentMidiSink;

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
      KeyEvent::new(KeyCode::Insert, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::Delete),
      KeyEvent::from(KeyCode::Enter),
      KeyEvent::from(KeyCode::Char('z')),
      KeyEvent::from(KeyCode::Char('x')),
      KeyEvent::from(KeyCode::Char('c')),
      KeyEvent::from(KeyCode::Char('v')),
      KeyEvent::from(KeyCode::Char('a')),
      KeyEvent::from(KeyCode::Char('w')),
      KeyEvent::from(KeyCode::Char('s')),
      KeyEvent::from(KeyCode::Char('e')),
      KeyEvent::from(KeyCode::Char('d')),
      KeyEvent::from(KeyCode::Char('f')),
      KeyEvent::from(KeyCode::Char('t')),
      KeyEvent::from(KeyCode::Char('g')),
      KeyEvent::from(KeyCode::Char('y')),
      KeyEvent::from(KeyCode::Char('h')),
      KeyEvent::from(KeyCode::Char('u')),
      KeyEvent::from(KeyCode::Char('j')),
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

    for column in [TrackColumn::Instrument, TrackColumn::Velocity] {
      state.focus.track_column = column;
      for digit in '0'..='9' {
        let result = handle_edit_pattern_event(
          &mut state,
          &mut pattern_scroll_state,
          &player,
          &mut midi,
          KeyEvent::from(KeyCode::Char(digit)),
        )
        .unwrap();
        assert!(result.handled, "{column:?} digit {digit:?} was not handled");
      }
    }
  }

  #[test]
  fn pattern_element_smoke_test() {
    let mut state = minimal_song();

    for key in [
      KeyEvent::from(KeyCode::Up),
      KeyEvent::from(KeyCode::Down),
      KeyEvent::from(KeyCode::PageUp),
      KeyEvent::from(KeyCode::PageDown),
      KeyEvent::from(KeyCode::Home),
      KeyEvent::from(KeyCode::End),
      KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::Enter),
      KeyEvent::from(KeyCode::Esc),
    ] {
      let result = handle_edit_pattern_element_event(&mut state, key);
      assert!(
        result.handled,
        "pattern element key {key:?} was not handled"
      );
    }
  }

  #[test]
  fn sequence_panel_smoke_test() {
    let mut state = minimal_song();
    let viewports = Viewports {
      sequence_height: 1,
      pattern_size: Size::new(1, 1),
    };

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
      KeyEvent::new(KeyCode::Insert, KeyModifiers::CONTROL),
      KeyEvent::from(KeyCode::Delete),
      KeyEvent::from(KeyCode::F(4)),
      KeyEvent::from(KeyCode::Enter),
    ] {
      let result = handle_edit_sequence_event(&mut state, viewports, key);
      assert!(result.handled, "sequence key {key:?} was not handled");
    }
  }

  #[test]
  fn editor_settings_smoke_test() {
    let mut state = minimal_song();
    for key in [
      KeyEvent::from(KeyCode::Up),
      KeyEvent::from(KeyCode::Down),
      KeyEvent::from(KeyCode::PageUp),
      KeyEvent::from(KeyCode::PageDown),
      KeyEvent::from(KeyCode::Left),
      KeyEvent::from(KeyCode::Right),
      KeyEvent::from(KeyCode::Home),
      KeyEvent::from(KeyCode::End),
      KeyEvent::from(KeyCode::Char('z')),
      KeyEvent::from(KeyCode::Char('x')),
      KeyEvent::from(KeyCode::Char('c')),
      KeyEvent::from(KeyCode::Char('v')),
      KeyEvent::from(KeyCode::Enter),
      KeyEvent::from(KeyCode::Esc),
    ] {
      let result = handle_edit_settings_event(&mut state, key, EditorSettingFocus::Octave);
      assert!(result.handled, "editor setting key {key:?} was not handled");
    }
  }

  #[test]
  fn rendering_smoke_test() {
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
  fn effect_keys_edit_existing_notes_without_preview_or_stepping() {
    let mut state = crate::state::test_state();
    state.focus.track_column = TrackColumn::Effect;
    let original = state.get_current_pattern().get_note(0, 0).unwrap();
    let mut midi = RecordingMidiSink(Vec::new());
    let mut scroll = ScrollViewState::default();
    let player = Player::default();

    for (key, expected) in [
      ('T', Some(Effect::SetTempo(TwoDigits::new(0).unwrap()))),
      ('9', Some(Effect::SetTempo(TwoDigits::new(9).unwrap()))),
      ('9', Some(Effect::SetTempo(TwoDigits::new(99).unwrap()))),
      ('d', Some(Effect::Delay(TwoDigits::new(0).unwrap()))),
      ('3', Some(Effect::Delay(TwoDigits::new(3).unwrap()))),
      ('0', Some(Effect::Delay(TwoDigits::new(30).unwrap()))),
      ('E', Some(Effect::Advance(TwoDigits::new(0).unwrap()))),
      ('0', Some(Effect::Advance(TwoDigits::new(0).unwrap()))),
      ('9', Some(Effect::Advance(TwoDigits::new(9).unwrap()))),
      ('t', Some(Effect::SetTempo(TwoDigits::new(0).unwrap()))),
      ('D', Some(Effect::Delay(TwoDigits::new(0).unwrap()))),
      ('e', Some(Effect::Advance(TwoDigits::new(0).unwrap()))),
    ] {
      let result = handle_edit_pattern_event(
        &mut state,
        &mut scroll,
        &player,
        &mut midi,
        KeyEvent::from(KeyCode::Char(key)),
      )
      .unwrap();
      assert!(result.handled);
      let note = state.get_current_pattern().get_note(0, 0).unwrap();
      assert_eq!(note.effect, expected);
      assert_eq!(note.octave, original.octave);
      assert_eq!(note.semitone, original.semitone);
      assert_eq!(note.instrument, original.instrument);
      assert_eq!(note.velocity, original.velocity);
      assert_eq!(state.focus.row_index, 0);
    }
    assert!(midi.0.is_empty());

    let result = handle_edit_pattern_event(
      &mut state,
      &mut scroll,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Char('.')),
    )
    .unwrap();
    assert!(!result.handled);
    assert_eq!(
      state.get_current_pattern().get_note(0, 0).unwrap().effect,
      Some(Effect::Advance(TwoDigits::new(0).unwrap()))
    );
  }

  #[test]
  fn delete_clears_effect_before_removing_note_only_in_effect_column() {
    for effect in [
      Effect::SetTempo(TwoDigits::new(20).unwrap()),
      Effect::Delay(TwoDigits::new(3).unwrap()),
      Effect::Advance(TwoDigits::new(3).unwrap()),
    ] {
      let mut state = crate::state::test_state();
      state.focus.track_column = TrackColumn::Effect;
      state.set_focused_note_effect(Some(effect));
      let original = state.get_current_pattern().get_note(0, 0).unwrap();
      let mut scroll = ScrollViewState::default();
      let player = Player::default();
      let mut midi = SilentMidiSink;

      handle_edit_pattern_event(
        &mut state,
        &mut scroll,
        &player,
        &mut midi,
        KeyEvent::from(KeyCode::Delete),
      )
      .unwrap();
      let note = state.get_current_pattern().get_note(0, 0).unwrap();
      assert_eq!(note.effect, None);
      assert_eq!(note.octave, original.octave);
      assert_eq!(note.semitone, original.semitone);
      assert_eq!(note.instrument, original.instrument);
      assert_eq!(note.velocity, original.velocity);
      assert_eq!(state.focus.row_index, 0);

      handle_edit_pattern_event(
        &mut state,
        &mut scroll,
        &player,
        &mut midi,
        KeyEvent::from(KeyCode::Char('4')),
      )
      .unwrap();
      assert_eq!(
        state.get_current_pattern().get_note(0, 0).unwrap().effect,
        None
      );

      handle_edit_pattern_event(
        &mut state,
        &mut scroll,
        &player,
        &mut midi,
        KeyEvent::from(KeyCode::Delete),
      )
      .unwrap();
      assert!(state.get_current_pattern().get_note(0, 0).is_none());
    }

    for column in [
      TrackColumn::Note,
      TrackColumn::Instrument,
      TrackColumn::Velocity,
    ] {
      let mut state = crate::state::test_state();
      state.focus.track_column = column;
      state.set_focused_note_effect(Some(Effect::SetTempo(TwoDigits::new(20).unwrap())));
      handle_edit_pattern_event(
        &mut state,
        &mut ScrollViewState::default(),
        &Player::default(),
        &mut SilentMidiSink,
        KeyEvent::from(KeyCode::Delete),
      )
      .unwrap();
      assert!(state.get_current_pattern().get_note(0, 0).is_none());
    }
  }

  #[test]
  fn effect_digits_do_not_create_notes() {
    let mut state = minimal_song();
    state.focus.track_column = TrackColumn::Effect;
    let mut midi = RecordingMidiSink(Vec::new());
    let mut scroll = ScrollViewState::default();
    let player = Player::default();
    for ch in ['1', '.'] {
      handle_edit_pattern_event(
        &mut state,
        &mut scroll,
        &player,
        &mut midi,
        KeyEvent::from(KeyCode::Char(ch)),
      )
      .unwrap();
    }
    assert!(state.get_current_pattern().get_note(0, 0).is_none());
    assert!(midi.0.is_empty());
    assert!(state.get_current_pattern().tracks.is_empty());

    handle_edit_pattern_event(
      &mut state,
      &mut scroll,
      &player,
      &mut midi,
      KeyEvent::from(KeyCode::Delete),
    )
    .unwrap();
    assert!(state.get_current_pattern().tracks.is_empty());
  }

  #[test]
  fn effect_keys_insert_notes_in_empty_cells() {
    for (ch, semitone) in [('t', 6), ('T', 6), ('d', 4), ('D', 4), ('e', 3), ('E', 3)] {
      for (stored_track, virtual_track) in [(false, false), (true, false), (true, true)] {
        let mut state = minimal_song();
        state.patterns[0].num_rows = std::num::NonZeroUsize::new(4).unwrap();
        if stored_track {
          state.patterns[0].tracks.push(Track { notes: vec![None] });
        }
        if virtual_track {
          state.track_titles.push("Virtual".to_owned());
          state.focus.track_index = 1;
        }
        state.focus.track_column = TrackColumn::Effect;
        state.editor_settings.octave = Octave(3);
        state.editor_settings.velocity = Velocity(70);
        state.editor_settings.step = 2;
        let track_index = state.focus.track_index;
        let mut midi = RecordingMidiSink(Vec::new());

        let result = handle_edit_pattern_event(
          &mut state,
          &mut ScrollViewState::default(),
          &Player::default(),
          &mut midi,
          KeyEvent::from(KeyCode::Char(ch)),
        )
        .unwrap();

        assert!(result.handled);
        let note = state
          .get_current_pattern()
          .get_note(0, track_index)
          .unwrap();
        assert_eq!(note.semitone, Semitone(semitone));
        assert_eq!(note.octave, state.editor_settings.octave);
        assert_eq!(note.instrument, state.editor_settings.instrument);
        assert_eq!(note.velocity, Velocity(70));
        assert_eq!(note.effect, None);
        assert_eq!(state.focus.row_index, 2);
        assert_eq!(state.focus.track_column, TrackColumn::Effect);
        assert_eq!(
          midi.0,
          vec![
            vec![MIDI_PROGRAM_CHANGE, note.instrument.0],
            vec![MIDI_NOTE_ON, 36 + semitone, note.midi_velocity()],
          ]
        );
      }
    }
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
        vec![MIDI_NOTE_ON, 36, note.midi_velocity()],
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
  fn inserting_a_note_snaps_velocity_before_preview_and_storage() {
    let mut state = test_state();
    state.editor_settings.instrument = InstrumentIndex(2);
    state.editor_settings.velocity = Velocity(70);
    let mut midi = RecordingMidiSink(Vec::new());
    let row_index = state.focus.row_index;
    let track_index = state.focus.track_index;

    handle_edit_pattern_event(
      &mut state,
      &mut ScrollViewState::default(),
      &Player::default(),
      &mut midi,
      KeyEvent::from(KeyCode::Char('a')),
    )
    .unwrap();

    let note = state
      .get_current_pattern()
      .get_note(row_index, track_index)
      .unwrap();
    assert_eq!(note.velocity, Velocity(60));
    assert_eq!(state.editor_settings.velocity, Velocity(70));
    assert_eq!(
      midi.0,
      vec![
        vec![MIDI_PROGRAM_CHANGE, note.instrument.0],
        vec![MIDI_NOTE_ON, 36, note.midi_velocity()],
      ]
    );
  }

  #[test]
  fn pattern_element_velocity_keys_move_between_layers() {
    use crate::state::instruments::{InstrumentMetadata, InstrumentName, Instruments};

    let mut state = minimal_song();
    state.instruments = Instruments::DirectMapped(vec![InstrumentMetadata {
      name: InstrumentName("Layered".to_owned()),
      velocity_boundaries: vec![0.2, 0.4, 0.8],
    }]);
    state.editor_settings.velocity = Velocity(80);
    state.set_focused_note(state.editor_settings.make_note(Semitone(0)));
    state.focus.track_column = TrackColumn::Velocity;

    for (key, expected) in [
      (KeyCode::Down, 60),
      (KeyCode::PageDown, 30),
      (KeyCode::Up, 60),
      (KeyCode::PageUp, 90),
    ] {
      handle_edit_pattern_element_event(&mut state, KeyEvent::from(key));
      assert_eq!(
        state.get_current_pattern().get_note(0, 0).unwrap().velocity,
        Velocity(expected)
      );
    }

    for digit in ['5', '5'] {
      handle_edit_pattern_event(
        &mut state,
        &mut ScrollViewState::default(),
        &Player::default(),
        &mut SilentMidiSink,
        KeyEvent::from(KeyCode::Char(digit)),
      )
      .unwrap();
    }
    assert_eq!(
      state.get_current_pattern().get_note(0, 0).unwrap().velocity,
      Velocity(55)
    );
    handle_edit_pattern_element_event(&mut state, KeyEvent::from(KeyCode::Up));
    assert_eq!(
      state.get_current_pattern().get_note(0, 0).unwrap().velocity,
      Velocity(60)
    );
  }

  #[test]
  fn pattern_element_keys_adjust_and_clamp_effects() {
    for constructor in [
      Effect::SetTempo as fn(TwoDigits) -> Effect,
      Effect::Delay,
      Effect::Advance,
    ] {
      let mut state = crate::state::test_state();
      state.focus.track_column = TrackColumn::Effect;
      state.set_focused_note_effect(Some(constructor(TwoDigits::new(20).unwrap())));
      let original = state.get_current_pattern().get_note(0, 0).unwrap();

      for (key, value) in [
        (KeyCode::Up, 21),
        (KeyCode::Down, 20),
        (KeyCode::PageUp, 30),
        (KeyCode::PageDown, 20),
        (KeyCode::Home, 0),
        (KeyCode::Down, 0),
        (KeyCode::PageDown, 0),
        (KeyCode::End, 99),
        (KeyCode::Up, 99),
        (KeyCode::PageUp, 99),
        (KeyCode::PageDown, 89),
      ] {
        assert!(handle_edit_pattern_element_event(&mut state, KeyEvent::from(key)).handled);
        let note = state.get_current_pattern().get_note(0, 0).unwrap();
        assert_eq!(
          note.effect,
          Some(constructor(TwoDigits::new(value).unwrap()))
        );
        assert_eq!(note.octave, original.octave);
        assert_eq!(note.semitone, original.semitone);
        assert_eq!(note.instrument, original.instrument);
        assert_eq!(note.velocity, original.velocity);
      }
    }
  }

  #[test]
  fn pattern_element_effect_keys_leave_missing_effects_and_notes_unchanged() {
    let mut state = crate::state::test_state();
    state.focus.track_column = TrackColumn::Effect;
    state.set_focused_note_effect(None);
    for key in [
      KeyCode::Up,
      KeyCode::Down,
      KeyCode::PageUp,
      KeyCode::PageDown,
      KeyCode::Home,
      KeyCode::End,
    ] {
      handle_edit_pattern_element_event(&mut state, KeyEvent::from(key));
      assert_eq!(
        state.get_current_pattern().get_note(0, 0).unwrap().effect,
        None
      );
    }
    state.remove_focused_note();
    handle_edit_pattern_element_event(&mut state, KeyEvent::from(KeyCode::Up));
    assert!(state.get_current_pattern().get_note(0, 0).is_none());
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
  fn ctrl_enter_copies_focused_note_attributes_to_editor_settings() {
    let mut state = test_state();
    state.set_focused_note(Note {
      octave: Octave(5),
      semitone: Semitone(7),
      instrument: InstrumentIndex(2),
      velocity: Velocity(81),
      effect: None,
    });
    state.editor_settings.step = 9;

    let result = handle_edit_pattern_element_event(
      &mut state,
      KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
    );

    assert!(result.handled);
    assert_eq!(result.new_mode, Some(UIMode::EditPattern));
    assert_eq!(state.editor_settings.octave, Octave(5));
    assert_eq!(state.editor_settings.velocity, Velocity(81));
    assert_eq!(state.editor_settings.instrument, InstrumentIndex(2));
    assert_eq!(state.editor_settings.step, 9);
  }

  #[test]
  fn ctrl_enter_copies_a_note_from_the_test_song() {
    let mut state = test_state();
    state.focus.row_index = 0;

    let result = handle_edit_pattern_element_event(
      &mut state,
      KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
    );

    assert!(result.handled);
    assert_eq!(state.editor_settings.octave, Octave(3));
    assert_eq!(state.editor_settings.instrument, InstrumentIndex(2));
    assert_eq!(state.editor_settings.velocity, Velocity(20));
  }

  #[test]
  fn ctrl_enter_without_a_focused_note_leaves_editor_settings_unchanged() {
    let mut state = test_state();
    let initial_settings = state.editor_settings;

    let result = handle_edit_pattern_element_event(
      &mut state,
      KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
    );

    assert!(result.handled);
    assert_eq!(result.new_mode, Some(UIMode::EditPattern));
    assert_eq!(state.editor_settings.octave, initial_settings.octave);
    assert_eq!(state.editor_settings.velocity, initial_settings.velocity);
    assert_eq!(
      state.editor_settings.instrument,
      initial_settings.instrument
    );
    assert_eq!(state.editor_settings.step, initial_settings.step);
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
    KeyCode::Enter if key.modifiers == KeyModifiers::CONTROL => {
      state.copy_focused_note_attributes_to_editor_settings();
      new_mode = Some(UIMode::EditPattern);
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
      context.player.stop();
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
    ViewportSync::PatternAndSequence => {
      scroll_cursor_into_view(state, pattern_scroll_state, viewports.pattern_size);
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

/// Polls for input until the next playback deadline or the maximum poll timeout.
///
/// In player mode, processes due playback events after polling, even when input
/// is ready. `now` and `poll` are injected to allow deterministic testing.
/// Returns `(input_ready, ticked)` and propagates polling or playback errors.
fn poll_and_process_playback(
  mode: &UIMode,
  state: &AppState,
  player: &mut Player,
  midi: &mut dyn MidiSink,
  mut now: impl FnMut() -> Instant,
  poll: impl FnOnce(Duration) -> io::Result<bool>,
) -> Result<(bool, bool)> {
  let playing = matches!(mode, UIMode::Player { .. });
  let timeout = if playing {
    player
      .next_deadline()
      .expect("Player mode requires active playback")
      .saturating_duration_since(now())
      .min(MAX_EVENT_POLL_TIMEOUT)
  } else {
    MAX_EVENT_POLL_TIMEOUT
  };
  let input_ready = poll(timeout)?;
  let ticked = if playing {
    let current_time = now();
    if current_time
      >= player
        .next_deadline()
        .expect("Player mode requires active playback")
    {
      player.tick(state, midi, current_time)?;
      true
    } else {
      false
    }
  } else {
    false
  };
  Ok((input_ready, ticked))
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

    let (input_ready, ticked) =
      poll_and_process_playback(&mode, &state, &mut player, midi, Instant::now, event::poll)?;
    if ticked {
      sync_player_position(
        &mut state,
        &player,
        &mut sequence_scroll_state,
        &mut pattern_scroll_state,
        viewports,
      );
    }

    if input_ready {
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
          player.stop();
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
    }
  }

  Ok(())
}
