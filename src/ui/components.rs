use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Line, Span, Style},
    widgets::Cell,
};

use crate::theme;

pub(super) const BAR_WIDTH: usize = 10;

pub(super) fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

pub(super) fn two_columns(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area)
        .to_vec()
}

pub(super) fn weighted_columns(area: Rect, left_percent: u16) -> Vec<Rect> {
    let left_percent = left_percent.min(100);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_percent),
            Constraint::Percentage(100 - left_percent),
        ])
        .split(area)
        .to_vec()
}

pub(super) fn bar_cell(value: u64) -> Cell<'static> {
    Cell::from(Line::from(heat_bar(value, BAR_WIDTH)))
}

fn heat_bar(value: u64, width: usize) -> Vec<Span<'static>> {
    let filled = ((value.min(100) as f64 / 100.0) * width as f64).ceil() as usize;
    let colors = [
        theme::BLUE,
        theme::BLUE_SOFT,
        theme::YELLOW_SOFT,
        theme::YELLOW,
        theme::ORANGE,
        theme::RED,
    ];

    (0..width)
        .map(|idx| {
            if idx < filled {
                let color_idx = idx * colors.len() / width.max(1);
                Span::styled(" ", Style::default().bg(colors[color_idx]))
            } else {
                Span::styled(" ", Style::default().bg(theme::BAR_EMPTY))
            }
        })
        .collect()
}
