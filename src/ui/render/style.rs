use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Block;

pub const DEFAULT_BG: Color = Color::Rgb(0, 0, 0);
pub const DEFAULT_FG: Color = Color::Rgb(230, 230, 230);
pub const TITLE_BG: Color = Color::Rgb(40, 80, 40);
pub const TITLE_STYLE: Style = Style::new()
  .bg(TITLE_BG)
  .fg(DEFAULT_FG)
  .add_modifier(Modifier::BOLD);
pub const SUB_TITLE_BG: Color = Color::Rgb(60, 60, 60);
pub const SUB_TITLE_STYLE: Style = Style::new().bg(SUB_TITLE_BG).fg(DEFAULT_FG);
pub const PANEL_AREA_STYLE: Style = Style::new().bg(Color::Rgb(35, 35, 35)).fg(DEFAULT_FG);
pub const FOCUSED_ROW_BG: Color = Color::Rgb(60, 60, 160);
pub const FOCUSED_ROW_STYLE: Style = Style::new().bg(FOCUSED_ROW_BG);
pub const INACTIVE_FOCUSED_ROW_BG: Color = Color::Rgb(80, 80, 120);
pub const INACTIVE_FOCUSED_ROW_STYLE: Style = Style::new().bg(INACTIVE_FOCUSED_ROW_BG);
pub const BEHIND_FOCUSED_ROW_STYLE: Style = Style::new().bg(Color::Rgb(48, 48, 120));
pub const BEHIND_INACTIVE_FOCUSED_ROW_STYLE: Style = Style::new().bg(Color::Rgb(65, 65, 90));

pub fn default_block() -> Block<'static> {
  Block::default().style(Style::default().bg(DEFAULT_BG).fg(DEFAULT_FG))
}
