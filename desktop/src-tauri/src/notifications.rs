use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tokenuse::{app::BackgroundUsageAlert, copy};

pub(crate) fn send_background_alert<R: Runtime>(
    app_handle: &AppHandle<R>,
    alert: BackgroundUsageAlert,
) {
    let _ = app_handle
        .notification()
        .builder()
        .title(copy::copy().brand.usage_alert_title.as_str())
        .body(background_alert_body(alert))
        .show();
}

fn background_alert_body(alert: BackgroundUsageAlert) -> String {
    let mut parts = Vec::new();
    if alert.cost_usd > 0.0 {
        parts.push(format!("${:.2}", alert.cost_usd));
    }
    if alert.tokens > 0 {
        parts.push(format!("{} tokens", format_compact_count(alert.tokens)));
    }
    if alert.calls > 0 {
        parts.push(format!(
            "{} {}",
            format_int(alert.calls),
            plural(alert.calls, "call", "calls")
        ));
    }

    let summary = if parts.is_empty() {
        copy::copy().status.background_usage_changed.clone()
    } else {
        parts.join(", ")
    };
    copy::template(
        &copy::copy().status.background_usage_body,
        &[("summary", summary)],
    )
}

fn format_compact_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format_int(n)
    }
}

fn format_int(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn plural<'a>(count: u64, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_alert_body_formats_usage_delta() {
        let body = background_alert_body(BackgroundUsageAlert {
            calls: 25,
            tokens: 120_000,
            cost_usd: 1.25,
        });

        assert_eq!(
            body,
            "Usage jumped by $1.25, 120.0K tokens, 25 calls since the last alert baseline."
        );
    }

    #[test]
    fn background_alert_body_skips_zero_delta_parts() {
        let body = background_alert_body(BackgroundUsageAlert {
            calls: 1,
            tokens: 0,
            cost_usd: 0.0,
        });

        assert_eq!(
            body,
            "Usage jumped by 1 call since the last alert baseline."
        );
    }
}
