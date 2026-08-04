use std::any::Any;
use std::io;
use std::panic::{self, AssertUnwindSafe};

use anyhow::Result;
use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{
  EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use dracker::app::run_app;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

struct TerminalCleanup {
  restored: bool,
}

impl TerminalCleanup {
  fn new() -> Self {
    Self { restored: false }
  }

  fn restore(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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

    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
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
  install_panic_hook();
  enable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen)?;
  let mut cleanup = TerminalCleanup::new();
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;

  let result = match panic::catch_unwind(AssertUnwindSafe(|| run_app(&mut terminal))) {
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
