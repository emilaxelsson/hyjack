use std::io;

use crossterm::event::{Event, KeyCode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::ui::common::{Component, ComponentAction};
use crate::ui::render::style::DEFAULT_FG;
use crate::ui::widgets::centered::Centered;

pub struct InputDialog<S> {
  pub title: String,
  pub label: String,
  pub input: String,
  pub cursor_index: usize,
  pub error: Option<String>,
  pub on_submit: fn(&mut S, String) -> anyhow::Result<()>,
}

/// Describes the visible slice of the input line and how the full logical cursor position
/// maps into that slice for rendering.
struct InputViewport {
  display_input: String,
  cursor_index: usize,
  window_start: usize,
}

impl<S> InputDialog<S> {
  fn block(&self) -> Block<'_> {
    Block::default()
      .title(self.title.as_str())
      .borders(Borders::ALL)
      .style(Style::default().bg(Color::Rgb(20, 20, 20)).fg(DEFAULT_FG))
  }

  /// Builds the horizontally scrolled view of the input line that fits inside the dialog,
  /// keeping the logical cursor position so rendering can place the terminal cursor over
  /// the visible slice.
  fn input_viewport(&self, inner_width: u16) -> InputViewport {
    let input_prefix = "> ";
    let max_input_width = inner_width
      .saturating_sub(input_prefix.chars().count() as u16)
      .saturating_sub(1) as usize;
    let input_chars = self.input.chars().collect::<Vec<_>>();
    let input_len = input_chars.len();
    let cursor_index = self.cursor_index.min(input_len);
    let window_start = if input_len <= max_input_width || cursor_index <= max_input_width {
      0
    } else {
      cursor_index - max_input_width
    };
    let window_end = if max_input_width == 0 {
      window_start
    } else {
      (window_start + max_input_width).min(input_len)
    };
    let display_input = input_chars[window_start..window_end]
      .iter()
      .collect::<String>();

    InputViewport {
      display_input,
      cursor_index,
      window_start,
    }
  }

  fn cursor_position(&self, area: Rect) -> (u16, u16) {
    let inner = self.block().inner(area);
    let viewport = self.input_viewport(inner.width);
    let input_prefix_width = "> ".chars().count() as u16;
    let cursor_x = inner
      .x
      .saturating_add(input_prefix_width)
      .saturating_add(viewport.cursor_index.saturating_sub(viewport.window_start) as u16);
    let cursor_y = inner.y.saturating_add(1);
    (cursor_x, cursor_y)
  }
}

impl<S> Widget for &InputDialog<S> {
  fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
    Clear.render(area, buf);

    let block = self.block();
    let inner = block.inner(area);
    let viewport = self.input_viewport(inner.width);

    block.render(area, buf);
    Paragraph::new(self.label.as_str()).render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
    Paragraph::new(format!("> {}", viewport.display_input)).render(
      Rect::new(inner.x, inner.y.saturating_add(1), inner.width, 1),
      buf,
    );
    Paragraph::new("Enter to confirm, Esc to cancel").render(
      Rect::new(inner.x, inner.y.saturating_add(2), inner.width, 1),
      buf,
    );

    if let Some(error) = self.error.as_deref() {
      Paragraph::new(error)
        .style(Style::default().fg(Color::Rgb(255, 120, 120)))
        .render(
          Rect::new(inner.x, inner.y.saturating_add(3), inner.width, 1),
          buf,
        );
    }
  }
}

impl<S> Component for InputDialog<S> {
  type State = S;

  fn draw(&self, frame: &mut Frame) {
    let centered = Centered::new(60, 20, self);
    let area = centered.inner_area(frame.area());
    let cursor = self.cursor_position(area);

    frame.render_widget(centered, frame.area());
    frame.set_cursor_position(cursor);
  }

  fn handle_event(&mut self, event: Event, state: &mut S) -> io::Result<ComponentAction> {
    let Event::Key(key) = event else {
      return Ok(ComponentAction::None);
    };

    match key.code {
      KeyCode::Esc => {
        return Ok(ComponentAction::Close);
      }
      KeyCode::Enter => match (self.on_submit)(state, self.input.clone()) {
        Ok(()) => {
          return Ok(ComponentAction::Close);
        }
        Err(err) => {
          self.error = Some(err.to_string());
        }
      },
      KeyCode::Backspace => {
        if self.cursor_index > 0 {
          let remove_start = char_to_byte_index(&self.input, self.cursor_index - 1);
          let remove_end = char_to_byte_index(&self.input, self.cursor_index);
          self.input.drain(remove_start..remove_end);
          self.cursor_index -= 1;
          self.error = None;
        }
      }
      KeyCode::Delete => {
        let len = self.input.chars().count();
        if self.cursor_index < len {
          let remove_start = char_to_byte_index(&self.input, self.cursor_index);
          let remove_end = char_to_byte_index(&self.input, self.cursor_index + 1);
          self.input.drain(remove_start..remove_end);
          self.error = None;
        }
      }
      KeyCode::Char(ch) => {
        let insert_at = char_to_byte_index(&self.input, self.cursor_index);
        self.input.insert(insert_at, ch);
        self.cursor_index += 1;
        self.error = None;
      }
      KeyCode::Left => {
        if self.cursor_index > 0 {
          self.cursor_index -= 1;
        }
      }
      KeyCode::Right => {
        let len = self.input.chars().count();
        if self.cursor_index < len {
          self.cursor_index += 1;
        }
      }
      KeyCode::Home => {
        self.cursor_index = 0;
      }
      KeyCode::End => {
        self.cursor_index = self.input.chars().count();
      }
      _ => {}
    }

    Ok(ComponentAction::None)
  }
}

fn char_to_byte_index(s: &str, char_index: usize) -> usize {
  if char_index == 0 {
    return 0;
  }

  s.char_indices()
    .nth(char_index)
    .map_or_else(|| s.len(), |(idx, _)| idx)
}
