use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::{env, io};

use anyhow::Result;
use crossterm::cursor::Show;
use crossterm::event::{
  KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
  EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
  supports_keyboard_enhancement,
};
use hyjack::app::run_app;
use hyjack::player::select_midi_sink;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

struct TerminalCleanup {
  restored: bool,
  keyboard_enhancement_enabled: bool,
}

impl TerminalCleanup {
  fn new() -> Self {
    Self {
      restored: false,
      keyboard_enhancement_enabled: false,
    }
  }

  fn restore(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    if self.keyboard_enhancement_enabled {
      execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
      self.keyboard_enhancement_enabled = false;
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    self.restored = true;
    Ok(())
  }

  fn restore_fallback(&mut self) {
    if self.restored {
      return;
    }

    let mut stdout = io::stdout();
    if self.keyboard_enhancement_enabled {
      let _ = execute!(stdout, PopKeyboardEnhancementFlags);
      self.keyboard_enhancement_enabled = false;
    }
    let _ = disable_raw_mode();
    let _ = execute!(stdout, LeaveAlternateScreen, Show);
    self.restored = true;
  }
}

impl Drop for TerminalCleanup {
  fn drop(&mut self) {
    self.restore_fallback();
  }
}

fn install_panic_hook() {
  let previous_hook = panic::take_hook();

  panic::set_hook(Box::new(move |panic_info| {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, Show);
    previous_hook(panic_info);
  }));
}

fn panic_payload_to_string(payload: &(dyn Any + Send)) -> String {
  if let Some(message) = payload.downcast_ref::<&str>() {
    (*message).to_string()
  } else if let Some(message) = payload.downcast_ref::<String>() {
    message.clone()
  } else {
    "unknown panic payload".to_string()
  }
}

fn main() -> Result<()> {
  let mut args = env::args_os();
  let program_name = args
    .next()
    .and_then(|name| name.into_string().ok())
    .unwrap_or_else(|| "hyjack".to_string());
  let song_file_path = match args.next() {
    Some(path) => PathBuf::from(path),
    None => {
      anyhow::bail!("usage: {program_name} <song-file-path>");
    }
  };

  if args.next().is_some() {
    anyhow::bail!("usage: {program_name} <song-file-path>");
  }

  let mut midi = select_midi_sink()?;
  install_panic_hook();
  enable_raw_mode()?;
  let mut cleanup = TerminalCleanup::new();
  let keyboard_enhancement_supported = match supports_keyboard_enhancement() {
    Ok(true) => true,
    Ok(false) => {
      eprintln!(
        "Warning: terminal does not support enhanced keyboard input; some shortcuts may not work"
      );
      false
    }
    Err(error) => {
      eprintln!("Warning: could not detect terminal keyboard enhancement support: {error}");
      false
    }
  };
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen)?;
  if keyboard_enhancement_supported {
    execute!(
      stdout,
      PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    cleanup.keyboard_enhancement_enabled = true;
  }
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;

  let result = match panic::catch_unwind(AssertUnwindSafe(|| {
    run_app(&mut terminal, &song_file_path, midi.as_mut())
  })) {
    Ok(result) => result,
    Err(payload) => Err(anyhow::anyhow!(
      "application panicked: {}",
      panic_payload_to_string(payload.as_ref())
    )),
  };

  // `restore` has better error handling than `restore_fallback` which is called in `Drop`
  cleanup.restore(&mut terminal)?;

  result
}
