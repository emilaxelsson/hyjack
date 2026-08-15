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
pub const INFO_BANNER_BG: Color = Color::Rgb(100, 100, 25);
pub const INFO_BANNER_STYLE: Style = Style::new()
  .bg(INFO_BANNER_BG)
  .fg(DEFAULT_FG)
  .add_modifier(Modifier::ITALIC);
pub const ERROR_BANNER_BG: Color = Color::Rgb(120, 25, 25);
pub const ERROR_BANNER_STYLE: Style = Style::new()
  .bg(ERROR_BANNER_BG)
  .fg(DEFAULT_FG)
  .add_modifier(Modifier::ITALIC);
pub const EDITABLE_FIELD_BG: Color = Color::Rgb(35, 35, 35);
pub const PANEL_AREA_STYLE: Style = Style::new().bg(EDITABLE_FIELD_BG).fg(DEFAULT_FG);
pub const FOCUSED_ROW_BG: Color = Color::Rgb(60, 60, 160);
pub const FOCUSED_ROW_STYLE: Style = Style::new().bg(FOCUSED_ROW_BG);
pub const INACTIVE_FOCUSED_ROW_BG: Color = Color::Rgb(80, 80, 120);
pub const INACTIVE_FOCUSED_ROW_STYLE: Style = Style::new().bg(INACTIVE_FOCUSED_ROW_BG);
pub const BEHIND_FOCUSED_ROW_STYLE: Style = Style::new().bg(Color::Rgb(48, 48, 120));
pub const BEHIND_INACTIVE_FOCUSED_ROW_STYLE: Style = Style::new().bg(Color::Rgb(65, 65, 90));
pub const SONG_END_BG: Color = Color::Rgb(100, 25, 25);
pub const SONG_END_FG: Color = Color::Rgb(180, 180, 180);
pub const SONG_END_STYLE: Style = Style::new().bg(SONG_END_BG).fg(SONG_END_FG);

pub fn default_block() -> Block<'static> {
  Block::default().style(Style::default().bg(DEFAULT_BG).fg(DEFAULT_FG))
}
