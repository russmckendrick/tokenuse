use crate::app::{Period, Tool};

pub(super) fn period_label(period: Period) -> &'static str {
    period.label()
}

pub(super) fn tool_label(tool: Tool) -> &'static str {
    tool.label()
}
