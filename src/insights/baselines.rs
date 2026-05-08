use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};

use crate::tools::ParsedCall;

const DAILY_WINDOW_DAYS: i64 = 30;
const SESSION_WINDOW_DAYS: i64 = 30;
const CACHE_RECENT_DAYS: i64 = 7;
const CACHE_PRIOR_DAYS: i64 = 30;

#[derive(Debug, Clone, Default)]
pub struct RollingStats {
    pub mean: f64,
    pub stdev: f64,
    pub n: usize,
}

impl RollingStats {
    pub fn from_samples(samples: &[f64]) -> Self {
        let n = samples.len();
        if n == 0 {
            return Self::default();
        }
        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n.max(1) as f64;
        Self {
            mean,
            stdev: variance.sqrt(),
            n,
        }
    }

    pub fn flagged_zscore(&self, value: f64, min_n: usize) -> Option<f64> {
        if self.n < min_n || self.stdev < 1e-9 {
            return None;
        }
        Some((value - self.mean) / self.stdev)
    }
}

#[derive(Debug, Clone, Default)]
pub struct OrderStats {
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
    pub iqr: f64,
    pub n: usize,
}

impl OrderStats {
    pub fn from_samples(mut samples: Vec<f64>) -> Self {
        let n = samples.len();
        if n == 0 {
            return Self::default();
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pick = |q: f64| -> f64 {
            let idx = ((q * (n - 1) as f64).round() as usize).min(n - 1);
            samples[idx]
        };
        let p25 = pick(0.25);
        let p50 = pick(0.50);
        let p75 = pick(0.75);
        let p95 = pick(0.95);
        Self {
            p50,
            p75,
            p95,
            iqr: (p75 - p25).max(0.0),
            n,
        }
    }

    pub fn outlier_threshold(&self) -> f64 {
        // Larger of P95 or Q3+1.5*IQR; protects against very narrow distributions.
        let upper = self.p75 + 1.5 * self.iqr;
        self.p95.max(upper)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HitRateWindow {
    pub last_7d: Option<f64>,
    pub prior_30d: Option<f64>,
    pub last_7d_input_tokens: u64,
    pub last_7d_cache_read_tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectMonthlyCost {
    pub current_month: f64,
    pub prior_month: f64,
    pub current_calls: u64,
    pub prior_calls: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CacheWriteRatio {
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub events: u64,
}

impl CacheWriteRatio {
    pub fn ratio(&self) -> Option<f64> {
        if self.cache_read_tokens == 0 {
            None
        } else {
            Some(self.cache_creation_tokens as f64 / self.cache_read_tokens as f64)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolMedian {
    pub median_hit_rate: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct Baselines {
    pub now: DateTime<Utc>,
    pub daily_cost_overall: RollingStats,
    pub session_cost_overall: OrderStats,
    pub cache_hit: HashMap<(&'static str, String), HitRateWindow>,
    pub project_monthly_cost: HashMap<String, ProjectMonthlyCost>,
    pub tool_cache_medians: HashMap<&'static str, ToolMedian>,
    pub cache_write_ratio_by_tool: HashMap<&'static str, CacheWriteRatio>,
}

impl Baselines {
    pub fn build(calls: &[ParsedCall], now: DateTime<Utc>) -> Self {
        let today = now.date_naive();

        // Daily cost over last 30 days.
        let mut daily_costs: HashMap<NaiveDate, f64> = HashMap::new();
        for call in calls {
            let Some(ts) = call.timestamp else { continue };
            let date = ts.date_naive();
            let age = (today - date).num_days();
            if (0..DAILY_WINDOW_DAYS).contains(&age) {
                *daily_costs.entry(date).or_default() += call.cost_usd;
            }
        }
        let daily_samples: Vec<f64> = daily_costs.values().copied().collect();
        let daily_cost_overall = RollingStats::from_samples(&daily_samples);

        // Session cost over last 30 days.
        let mut session_costs: HashMap<String, f64> = HashMap::new();
        for call in calls {
            let Some(ts) = call.timestamp else { continue };
            let age = (today - ts.date_naive()).num_days();
            if !(0..SESSION_WINDOW_DAYS).contains(&age) {
                continue;
            }
            if call.session_id.is_empty() {
                continue;
            }
            *session_costs.entry(call.session_id.clone()).or_default() += call.cost_usd;
        }
        let session_cost_overall =
            OrderStats::from_samples(session_costs.values().copied().collect());

        // Cache hit windows per (tool, project) — only for tools that report cache.
        let mut cache_recent: HashMap<(&'static str, String), (u64, u64)> = HashMap::new();
        let mut cache_prior: HashMap<(&'static str, String), (u64, u64)> = HashMap::new();
        for call in calls {
            if !tool_reports_cache(call.tool) {
                continue;
            }
            let Some(ts) = call.timestamp else { continue };
            let age = (today - ts.date_naive()).num_days();
            let key = (call.tool, call.project.clone());
            let bucket = if (0..CACHE_RECENT_DAYS).contains(&age) {
                Some(&mut cache_recent)
            } else if (CACHE_RECENT_DAYS..CACHE_PRIOR_DAYS + CACHE_RECENT_DAYS).contains(&age) {
                Some(&mut cache_prior)
            } else {
                None
            };
            if let Some(bucket) = bucket {
                let entry = bucket.entry(key).or_default();
                entry.0 += call.input_tokens + call.cache_read_input_tokens;
                entry.1 += call.cache_read_input_tokens;
            }
        }
        let mut cache_hit: HashMap<(&'static str, String), HitRateWindow> = HashMap::new();
        let mut all_keys: std::collections::HashSet<(&'static str, String)> =
            cache_recent.keys().cloned().collect();
        all_keys.extend(cache_prior.keys().cloned());
        for key in all_keys {
            let recent = cache_recent.get(&key).copied();
            let prior = cache_prior.get(&key).copied();
            let last_7d = recent.and_then(|(input_total, reads)| {
                if input_total == 0 {
                    None
                } else {
                    Some(reads as f64 / input_total as f64)
                }
            });
            let prior_30d = prior.and_then(|(input_total, reads)| {
                if input_total == 0 {
                    None
                } else {
                    Some(reads as f64 / input_total as f64)
                }
            });
            let (input, reads) = recent.unwrap_or_default();
            cache_hit.insert(
                key,
                HitRateWindow {
                    last_7d,
                    prior_30d,
                    last_7d_input_tokens: input.saturating_sub(reads),
                    last_7d_cache_read_tokens: reads,
                },
            );
        }

        // Tool-wide median cache hit rates from the last 7 days.
        let mut by_tool: HashMap<&'static str, Vec<f64>> = HashMap::new();
        for ((tool, _project), window) in &cache_hit {
            if let Some(rate) = window.last_7d {
                by_tool.entry(*tool).or_default().push(rate);
            }
        }
        let mut tool_cache_medians: HashMap<&'static str, ToolMedian> = HashMap::new();
        for (tool, mut rates) in by_tool {
            rates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = if rates.is_empty() {
                None
            } else {
                Some(rates[rates.len() / 2])
            };
            tool_cache_medians.insert(
                tool,
                ToolMedian {
                    median_hit_rate: median,
                },
            );
        }

        // Project monthly cost — current vs prior calendar month.
        let mut project_monthly_cost: HashMap<String, ProjectMonthlyCost> = HashMap::new();
        let prior_month_anchor = today - Duration::days(today.day0() as i64 + 1);
        for call in calls {
            let Some(ts) = call.timestamp else { continue };
            let date = ts.date_naive();
            if date.year() == today.year() && date.month() == today.month() {
                let entry = project_monthly_cost
                    .entry(call.project.clone())
                    .or_default();
                entry.current_month += call.cost_usd;
                entry.current_calls += 1;
            } else if date.year() == prior_month_anchor.year()
                && date.month() == prior_month_anchor.month()
            {
                let entry = project_monthly_cost
                    .entry(call.project.clone())
                    .or_default();
                entry.prior_month += call.cost_usd;
                entry.prior_calls += 1;
            }
        }

        // Cache write/read ratio per tool (last 7 days, cache-reporting tools only).
        let mut cache_write_ratio_by_tool: HashMap<&'static str, CacheWriteRatio> = HashMap::new();
        for call in calls {
            if !tool_reports_cache(call.tool) {
                continue;
            }
            let Some(ts) = call.timestamp else { continue };
            let age = (today - ts.date_naive()).num_days();
            if !(0..CACHE_RECENT_DAYS).contains(&age) {
                continue;
            }
            let entry = cache_write_ratio_by_tool.entry(call.tool).or_default();
            entry.cache_creation_tokens += call.cache_creation_input_tokens;
            entry.cache_read_tokens += call.cache_read_input_tokens;
            entry.events += 1;
        }

        Self {
            now,
            daily_cost_overall,
            session_cost_overall,
            cache_hit,
            project_monthly_cost,
            tool_cache_medians,
            cache_write_ratio_by_tool,
        }
    }

    pub fn today_total(&self, calls: &[ParsedCall]) -> f64 {
        let today = self.now.date_naive();
        calls
            .iter()
            .filter(|c| c.timestamp.map(|ts| ts.date_naive()) == Some(today))
            .map(|c| c.cost_usd)
            .sum()
    }
}

pub fn tool_reports_cache(tool: &str) -> bool {
    matches!(tool, "claude-code" | "codex" | "gemini")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(days_ago: i64) -> Option<DateTime<Utc>> {
        let now = Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0).unwrap();
        Some(now - Duration::days(days_ago))
    }

    fn call(tool: &'static str, project: &str, days_ago: i64, cost: f64) -> ParsedCall {
        ParsedCall {
            tool,
            project: project.into(),
            session_id: format!("{project}-{days_ago}"),
            cost_usd: cost,
            input_tokens: 1_000,
            timestamp: ts(days_ago),
            ..ParsedCall::default()
        }
    }

    #[test]
    fn rolling_stats_handles_empty() {
        let stats = RollingStats::from_samples(&[]);
        assert_eq!(stats.n, 0);
        assert!(stats.flagged_zscore(10.0, 1).is_none());
    }

    #[test]
    fn rolling_stats_returns_none_below_min_n() {
        let stats = RollingStats::from_samples(&[1.0, 2.0, 3.0]);
        assert!(stats.flagged_zscore(10.0, 14).is_none());
    }

    #[test]
    fn order_stats_outlier_threshold_uses_p95_or_iqr() {
        let stats = OrderStats::from_samples(vec![1.0, 2.0, 3.0, 4.0, 100.0]);
        assert!(stats.outlier_threshold() >= stats.p95);
    }

    #[test]
    fn baselines_compute_daily_and_session_stats() {
        let now = Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0).unwrap();
        let calls = vec![
            call("claude-code", "alpha", 1, 1.0),
            call("claude-code", "alpha", 2, 1.5),
            call("claude-code", "beta", 3, 2.0),
            call("claude-code", "alpha", 4, 0.5),
            call("claude-code", "alpha", 60, 99.0), // outside window
        ];
        let baselines = Baselines::build(&calls, now);
        assert_eq!(baselines.daily_cost_overall.n, 4);
        assert_eq!(baselines.session_cost_overall.n, 4);
    }

    #[test]
    fn cache_baselines_skip_tools_without_cache_data() {
        let now = Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0).unwrap();
        let mut a = call("cursor", "alpha", 1, 1.0);
        a.cache_read_input_tokens = 500;
        let baselines = Baselines::build(&[a], now);
        assert!(baselines.cache_hit.is_empty());
    }
}
