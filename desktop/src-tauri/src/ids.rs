use tokenuse::{
    app::{Page, Period, SortMode, StatusTone, Tool},
    export::ExportFormat,
};

use crate::state::{unknown, CommandResult};

pub(crate) fn status_tone_id(tone: StatusTone) -> &'static str {
    match tone {
        StatusTone::Info => "info",
        StatusTone::Busy => "busy",
        StatusTone::Success => "success",
        StatusTone::Warning => "warning",
        StatusTone::Error => "error",
    }
}

pub(crate) fn parse_page(value: &str) -> CommandResult<Page> {
    match value {
        "overview" => Ok(Page::Overview),
        "deep-dive" => Ok(Page::DeepDive),
        "usage" => Ok(Page::Usage),
        "config" => Ok(Page::Config),
        "session" => Ok(Page::Session),
        _ => Err(unknown("page", value)),
    }
}

pub(crate) fn page_id(page: Page) -> &'static str {
    match page {
        Page::Overview => "overview",
        Page::DeepDive => "deep-dive",
        Page::Config => "config",
        Page::Usage => "usage",
        Page::Session => "session",
    }
}

pub(crate) fn parse_period(value: &str) -> CommandResult<Period> {
    match value {
        "today" => Ok(Period::Today),
        "week" => Ok(Period::Week),
        "thirty-days" => Ok(Period::ThirtyDays),
        "month" => Ok(Period::Month),
        "all-time" => Ok(Period::AllTime),
        _ => Err(unknown("period", value)),
    }
}

pub(crate) fn period_id(period: Period) -> &'static str {
    match period {
        Period::Today => "today",
        Period::Week => "week",
        Period::ThirtyDays => "thirty-days",
        Period::Month => "month",
        Period::AllTime => "all-time",
    }
}

pub(crate) fn parse_tool(value: &str) -> CommandResult<Tool> {
    match value {
        "all" => Ok(Tool::All),
        "claude-code" => Ok(Tool::ClaudeCode),
        "cursor" => Ok(Tool::Cursor),
        "codex" => Ok(Tool::Codex),
        "copilot" => Ok(Tool::Copilot),
        "gemini" => Ok(Tool::Gemini),
        _ => Err(unknown("tool", value)),
    }
}

pub(crate) fn tool_id(tool: Tool) -> &'static str {
    match tool {
        Tool::All => "all",
        Tool::ClaudeCode => "claude-code",
        Tool::Cursor => "cursor",
        Tool::Codex => "codex",
        Tool::Copilot => "copilot",
        Tool::Gemini => "gemini",
    }
}

pub(crate) fn parse_sort(value: &str) -> CommandResult<SortMode> {
    match value {
        "spend" => Ok(SortMode::Spend),
        "date" => Ok(SortMode::Date),
        "tokens" => Ok(SortMode::Tokens),
        _ => Err(unknown("sort", value)),
    }
}

pub(crate) fn sort_id(sort: SortMode) -> &'static str {
    match sort {
        SortMode::Spend => "spend",
        SortMode::Date => "date",
        SortMode::Tokens => "tokens",
    }
}

pub(crate) fn parse_export_format(value: &str) -> CommandResult<ExportFormat> {
    match value {
        "json" => Ok(ExportFormat::Json),
        "csv" => Ok(ExportFormat::Csv),
        "svg" => Ok(ExportFormat::Svg),
        "png" => Ok(ExportFormat::Png),
        "html" => Ok(ExportFormat::Html),
        "pdf" => Ok(ExportFormat::Pdf),
        _ => Err(unknown("export format", value)),
    }
}

pub(crate) fn export_format_id(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
        ExportFormat::Svg => "svg",
        ExportFormat::Png => "png",
        ExportFormat::Html => "html",
        ExportFormat::Pdf => "pdf",
    }
}
