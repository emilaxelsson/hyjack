use ratatui::style::{Color, Modifier, Style};

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
pub const ACTIVE_ROW_BG: Color = Color::Rgb(55, 55, 150);
pub const ACTIVE_ROW_STYLE: Style = Style::new().bg(ACTIVE_ROW_BG);
pub const BEHIND_ACTIVE_ROW_STYLE: Style = Style::new().bg(Color::Rgb(45, 45, 120));
