use ratatui::{
    prelude::{Color, Modifier, Style},
    widgets::{Block, Borders},
};

pub const BACKGROUND: Color = Color::Rgb(32, 36, 56);
pub const SURFACE: Color = Color::Rgb(37, 41, 61);
pub const BAR_EMPTY: Color = Color::Rgb(41, 45, 66);
pub const TEXT: Color = Color::Rgb(203, 212, 242);
pub const MUTED: Color = Color::Rgb(161, 167, 195);
pub const DIM: Color = Color::Rgb(110, 116, 146);
pub const PRIMARY: Color = Color::Rgb(255, 143, 64);
pub const BLUE: Color = Color::Rgb(98, 166, 255);
pub const BLUE_SOFT: Color = Color::Rgb(126, 188, 255);
pub const GREEN: Color = Color::Rgb(76, 242, 160);
pub const YELLOW: Color = Color::Rgb(255, 214, 10);
pub const YELLOW_SOFT: Color = Color::Rgb(245, 207, 108);
pub const ORANGE: Color = Color::Rgb(255, 156, 72);
pub const RED: Color = Color::Rgb(255, 95, 109);
pub const CYAN: Color = Color::Rgb(77, 243, 232);
pub const MAGENTA: Color = Color::Rgb(240, 90, 242);

pub fn base() -> Style {
    Style::default().fg(TEXT).bg(BACKGROUND)
}

pub fn muted() -> Style {
    base().fg(MUTED)
}

pub fn dim() -> Style {
    base().fg(DIM)
}

pub fn money() -> Style {
    base().fg(YELLOW)
}

pub fn key() -> Style {
    base().fg(PRIMARY).add_modifier(Modifier::BOLD)
}

pub fn panel_block<'a>(title: &'a str, color: Color) -> Block<'a> {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color).bg(BACKGROUND))
        .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BACKGROUND));

    if title.is_empty() {
        block
    } else {
        block.title(format!(" {title} "))
    }
}
