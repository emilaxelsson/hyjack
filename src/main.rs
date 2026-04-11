use std::{io, time::Duration};

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
  EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table};
use ratatui::{Frame, Terminal};

use dracker::dom::*;

struct TerminalCleanup {
  restored: bool,
}

impl TerminalCleanup {
  fn new() -> Self {
    Self { restored: false }
  }

  fn restore(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
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

fn main() -> io::Result<()> {
  enable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen)?;
  let mut cleanup = TerminalCleanup::new();
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;

  let result = run_app(&mut terminal);

  // `restore` has better error handling than `restore_fallback` which is called in `Drop`
  cleanup.restore(&mut terminal)?;

  result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
  loop {
    terminal.draw(draw_ui)?;

    if event::poll(Duration::from_millis(100))?
      && let Event::Key(key) = event::read()?
      && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
    {
      break;
    }
  }

  Ok(())
}

fn draw_ui(frame: &mut Frame) {
  let root = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Length(1),
      Constraint::Min(15),
      Constraint::Length(1),
      Constraint::Length(1),
    ])
    .split(frame.area());

  frame.render_widget(
    Paragraph::new("Dracker 0.2    00:02:32.000 | My song - Intro A"),
    root[0],
  );

  frame.render_widget(horizontal_rule(root[1].width), root[1]);

  frame.render_widget(
    Paragraph::new("Editing        Octave 3 | Velocity 50 | Step  4 | Instrument  2/Hihat"),
    root[2],
  );

  frame.render_widget(horizontal_rule(root[3].width), root[3]);

  let body = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Length(22), Constraint::Min(40)])
    .split(root[4]);

  draw_sequence_panel(frame, body[0]);
  draw_pattern_panel(frame, body[1]);

  frame.render_widget(
    Paragraph::new("Note  2/Hihat").alignment(Alignment::Center),
    root[5],
  );
  frame.render_widget(
    Paragraph::new("Pattern  Beats  4 | RPB  4").alignment(Alignment::Center),
    root[6],
  );
}

fn horizontal_rule(width: u16) -> Paragraph<'static> {
  let rule = "─".repeat(width as usize);
  Paragraph::new(rule)
}

fn draw_sequence_panel(frame: &mut Frame, area: ratatui::layout::Rect) {
  let items = vec![
    ListItem::new(" Intro A"),
    ListItem::new(" Intro B"),
    ListItem::new(" Verse 1 A"),
    ListItem::new(" Verse 1 B"),
    ListItem::new(" Verse 1 C"),
    ListItem::new(" Verse 1 D"),
    ListItem::new(" Bridge A"),
    ListItem::new(" Bridge B"),
    ListItem::new(""),
    ListItem::new(" - Pool -----------"),
    ListItem::new(" Bridge B"),
    ListItem::new(" Intro X"),
  ];

  let list = List::new(items)
    .block(Block::default().title(" Sequence ").borders(Borders::ALL))
    .highlight_style(Style::default().add_modifier(Modifier::BOLD));

  frame.render_widget(list, area);
}

fn draw_pattern_panel(frame: &mut Frame, area: ratatui::layout::Rect) {
  let rows = vec![
    Row::new(vec![
      Cell::from("C-3 02 v20  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("C-3 02 v20  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
    Row::new(vec![
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
      Cell::from("... ..  ..  ..."),
    ]),
  ];

  let table = Table::new(
    rows,
    [
      Constraint::Percentage(25),
      Constraint::Percentage(25),
      Constraint::Percentage(25),
      Constraint::Percentage(25),
    ],
  )
  .header(Row::new(vec!["Hat/ride", "Kick", "Snare", "Cymbals"]))
  .column_spacing(1)
  .block(
    Block::default()
      .title(Line::from(" Intro A "))
      .borders(Borders::ALL),
  );

  frame.render_widget(table, area);
}
