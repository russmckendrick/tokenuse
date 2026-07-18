//! Scriptable CLI summaries: `tokenuse status` (one-liner) and
//! `tokenuse overview` (copy-pasteable monthly summary). Both sync the
//! archive the same way `--list-projects` does, render plain deterministic
//! text (no ANSI), and take `--json` for machine output.

use color_eyre::Result;
use serde::Serialize;

use crate::app::{ModelFilter, Period, ProjectFilter, SortMode, Tool};
use crate::archive;
use crate::config::{ConfigPaths, UserConfig};
use crate::copy::{copy, template};
use crate::currency::{CurrencyFormatter, CurrencyTable};
use crate::data::DashboardData;
use crate::ingest::{self, Ingested, PeriodTotals};

struct CliData {
    ingested: Ingested,
    formatter: CurrencyFormatter,
}

fn load_data() -> Result<Option<CliData>> {
    let paths = ConfigPaths::default();
    let config = UserConfig::load(&paths).unwrap_or_default();
    let currency_table = CurrencyTable::load(&paths).unwrap_or_else(|_| {
        CurrencyTable::embedded().expect("embedded currency rates must be valid JSON")
    });
    let formatter = currency_table.formatter(&config.currency);

    let ingested = match archive::sync_and_load(&paths) {
        Ok(ingested) => ingested,
        Err(error) => {
            eprintln!(
                "{}",
                template(
                    &copy().cli.archive_failed_raw_ingest,
                    &[("error", error.to_string())]
                )
            );
            ingest::load()?
        }
    };
    if ingested.is_empty() {
        return Ok(None);
    }
    Ok(Some(CliData {
        ingested,
        formatter,
    }))
}

#[derive(Serialize)]
struct StatusJson<'a> {
    currency: &'a str,
    today: PeriodJson,
    month: PeriodJson,
}

#[derive(Serialize)]
struct PeriodJson {
    cost: String,
    #[serde(flatten)]
    totals: PeriodTotals,
}

fn period_json(totals: PeriodTotals, formatter: &CurrencyFormatter) -> PeriodJson {
    PeriodJson {
        cost: formatter.format_money(totals.cost_usd),
        totals,
    }
}

pub fn run_status(json: bool) -> Result<()> {
    let Some(data) = load_data()? else {
        println!("{}", copy().cli.no_local_sessions_found);
        return Ok(());
    };
    let today = data.ingested.period_totals(Period::Today);
    let month = data.ingested.period_totals(Period::Month);

    if json {
        let payload = StatusJson {
            currency: data.formatter.code(),
            today: period_json(today, &data.formatter),
            month: period_json(month, &data.formatter),
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!(
        "{}",
        template(
            &copy().overview.status_line,
            &[
                ("today_cost", data.formatter.format_money(today.cost_usd)),
                ("today_calls", today.calls.to_string()),
                ("today_sessions", today.sessions.to_string()),
                ("month_cost", data.formatter.format_money(month.cost_usd)),
                ("month_calls", month.calls.to_string()),
                ("month_sessions", month.sessions.to_string()),
            ]
        )
    );
    Ok(())
}

#[derive(Serialize)]
struct OverviewJson<'a> {
    period: &'a str,
    currency: &'a str,
    totals: PeriodJson,
    by_tool: Vec<ToolJson>,
    dashboard: &'a DashboardData,
}

#[derive(Serialize)]
struct ToolJson {
    tool: &'static str,
    cost: String,
    #[serde(flatten)]
    totals: PeriodTotals,
}

pub fn run_overview(json: bool) -> Result<()> {
    let Some(data) = load_data()? else {
        println!("{}", copy().cli.no_local_sessions_found);
        return Ok(());
    };
    let period = Period::Month;
    let totals = data.ingested.period_totals(period);
    let by_tool = data.ingested.tool_totals(period);
    let dashboard = data.ingested.dashboard(
        period,
        Tool::All,
        &ProjectFilter::All,
        &ModelFilter::All,
        SortMode::Spend,
        &data.formatter,
    );

    if json {
        let payload = OverviewJson {
            period: &copy().periods.month,
            currency: data.formatter.code(),
            totals: period_json(totals, &data.formatter),
            by_tool: by_tool
                .into_iter()
                .map(|(tool, totals)| ToolJson {
                    tool,
                    cost: data.formatter.format_money(totals.cost_usd),
                    totals,
                })
                .collect(),
            dashboard: &dashboard,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    print!(
        "{}",
        render_overview(&data.formatter, totals, &by_tool, &dashboard)
    );
    Ok(())
}

const TOP_ROWS: usize = 8;

fn render_overview(
    formatter: &CurrencyFormatter,
    totals: PeriodTotals,
    by_tool: &[(&'static str, PeriodTotals)],
    dashboard: &DashboardData,
) -> String {
    let c = copy();
    let mut out = String::new();

    out.push_str(&format!(
        "{}  {} ({})\n\n",
        c.brand.command,
        c.periods.month,
        formatter.code()
    ));

    // Totals
    out.push_str(&format!("{}\n", c.overview.totals_heading));
    out.push_str(&format!(
        "  {:<10} {}\n",
        c.metrics.cost,
        formatter.format_money(totals.cost_usd)
    ));
    out.push_str(&format!(
        "  {:<10} {}   {} {}   {} {}\n",
        c.metrics.calls,
        totals.calls,
        c.metrics.sessions,
        totals.sessions,
        c.metrics.cache_hit,
        dashboard.summary.cache_hit
    ));
    out.push_str(&format!(
        "  {:<10} {} {} / {} {} / {} {} / {} {}\n\n",
        c.metrics.tokens,
        c.metrics.input,
        dashboard.summary.input,
        c.metrics.output,
        dashboard.summary.output,
        c.metrics.cached,
        dashboard.summary.cached,
        c.metrics.written,
        dashboard.summary.written
    ));

    // By tool
    if !by_tool.is_empty() {
        out.push_str(&format!("{}\n", c.overview.by_tool_heading));
        let width = by_tool.iter().map(|(t, _)| t.len()).max().unwrap_or(4);
        for (tool, tool_totals) in by_tool {
            out.push_str(&format!(
                "  {:<width$}  {:>10}  {} {}\n",
                tool,
                formatter.format_money(tool_totals.cost_usd),
                tool_totals.calls,
                c.panels.calls.to_lowercase(),
            ));
        }
        out.push('\n');
    }

    if !dashboard.by_activity.is_empty() {
        out.push_str(&format!(
            "{}\n",
            c.categories
                .get("heading")
                .map(String::as_str)
                .unwrap_or("By Activity")
        ));
        for activity in &dashboard.by_activity {
            out.push_str(&format!(
                "  {:>10}  {} ({} {})\n",
                activity.cost,
                activity.label,
                activity.calls,
                c.panels.calls.to_lowercase()
            ));
        }
        out.push('\n');
    }

    // Top models / top projects from the display-formatted dashboard rows.
    if !dashboard.models.is_empty() {
        out.push_str(&format!("{}\n", c.panels.by_model));
        for model in dashboard.models.iter().take(TOP_ROWS) {
            out.push_str(&format!("  {:>10}  {}\n", model.cost, model.name));
        }
        out.push('\n');
    }
    if !dashboard.projects.is_empty() {
        out.push_str(&format!("{}\n", c.panels.by_project));
        for project in dashboard.projects.iter().take(TOP_ROWS) {
            out.push_str(&format!("  {:>10}  {}\n", project.cost, project.name));
        }
        out.push('\n');
    }
    if !dashboard.daily.is_empty() {
        out.push_str(&format!("{}\n", c.panels.daily_activity));
        // Chronological regardless of the dashboard sort: a pasted summary
        // reads as a timeline.
        let mut days: Vec<_> = dashboard.daily.iter().collect();
        days.sort_by(|a, b| a.day.cmp(b.day));
        for day in days {
            out.push_str(&format!(
                "  {}  {:>10}  {} {}\n",
                day.day,
                day.cost,
                day.calls,
                c.panels.calls.to_lowercase()
            ));
        }
    }

    out
}
