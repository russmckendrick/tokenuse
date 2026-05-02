use ratatui::{
    prelude::{Color, Line, Span},
    widgets::Cell,
};

use crate::theme;

pub(super) const RANK_WIDTH: usize = 8;

const BLOCKS: [&str; 9] = ["·", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub(super) fn rank_cell(value: u64) -> Cell<'static> {
    Cell::from(Line::from(ranked_bar_spans(value, RANK_WIDTH)))
}

pub(super) fn gauge_cell(value: u64) -> Cell<'static> {
    Cell::from(Line::from(gauge_spans(
        value,
        RANK_WIDTH,
        gauge_color(value),
    )))
}

pub(super) fn sparkline_spans(values: &[u64], width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    if values.is_empty() || values.iter().all(|value| *value == 0) {
        return vec![Span::styled("·".repeat(width), theme::dim())];
    }

    scaled_levels(values, width, 8)
        .into_iter()
        .map(|level| {
            let style = if level == 0 {
                theme::dim()
            } else if level < 4 {
                theme::base().fg(theme::BLUE_SOFT)
            } else if level < 7 {
                theme::base().fg(theme::CYAN)
            } else {
                theme::key()
            };
            Span::styled(BLOCKS[level as usize], style)
        })
        .collect()
}

pub(super) fn ranked_bar_spans(value: u64, width: usize) -> Vec<Span<'static>> {
    let filled = filled_cells(value, width);
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
                Span::styled("█", theme::base().fg(colors[color_idx]))
            } else {
                Span::styled("·", theme::dim())
            }
        })
        .collect()
}

pub(super) fn gauge_spans(value: u64, width: usize, color: Color) -> Vec<Span<'static>> {
    let filled = filled_cells(value, width);
    (0..width)
        .map(|idx| {
            if idx < filled {
                Span::styled("█", theme::base().fg(color))
            } else {
                Span::styled("░", theme::dim())
            }
        })
        .collect()
}

pub(super) fn no_data_line(label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, theme::key()),
        Span::styled(" no data", theme::dim()),
    ])
}

fn gauge_color(value: u64) -> Color {
    match value.min(100) {
        0..=49 => theme::CYAN,
        50..=79 => theme::YELLOW,
        _ => theme::RED,
    }
}

fn filled_cells(value: u64, width: usize) -> usize {
    if width == 0 || value == 0 {
        return 0;
    }
    ((value.min(100) as f64 / 100.0) * width as f64).ceil() as usize
}

fn scaled_levels(values: &[u64], width: usize, levels: u64) -> Vec<u64> {
    let sampled = downsample(values, width);
    let max = sampled.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return vec![0; sampled.len()];
    }

    sampled
        .into_iter()
        .map(|value| {
            if value == 0 {
                0
            } else {
                ((value as f64 / max as f64) * levels as f64)
                    .ceil()
                    .clamp(1.0, levels as f64) as u64
            }
        })
        .collect()
}

fn downsample(values: &[u64], width: usize) -> Vec<u64> {
    if width == 0 || values.is_empty() {
        return Vec::new();
    }
    if values.len() == width {
        return values.to_vec();
    }
    if values.len() < width {
        return (0..width)
            .map(|idx| values[idx * values.len() / width])
            .collect();
    }

    let len = values.len();
    (0..width)
        .map(|idx| {
            let start = idx * len / width;
            let end = if idx + 1 == width {
                len
            } else {
                ((idx + 1) * len / width).max(start + 1)
            };
            values[start..end].iter().copied().max().unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_levels_keep_zero_values_empty() {
        assert_eq!(scaled_levels(&[0, 5, 10], 3, 8), vec![0, 4, 8]);
    }

    #[test]
    fn sparkline_spans_render_empty_values_as_muted_dots() {
        let spans = sparkline_spans(&[], 6);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "······");
    }

    #[test]
    fn downsample_uses_maxima_for_each_bucket() {
        assert_eq!(downsample(&[1, 9, 2, 8, 3, 7, 4, 6], 4), vec![9, 8, 7, 6]);
    }
}
