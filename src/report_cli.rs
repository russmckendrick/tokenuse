use std::{
    collections::BTreeSet,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use color_eyre::Result;
use tokenuse::{
    app::{Period, SortMode, Tool},
    archive,
    config::{ConfigPaths, UserConfig},
    copy::{copy, template},
    currency::{CurrencyFormatter, CurrencyTable},
    data::ProjectOption,
    ingest::{self, Ingested},
    reports::{self, ReportBatchRequest, ReportFormat, ReportResponse, ReportScope},
};

pub fn run() -> Result<()> {
    let paths = ConfigPaths::default();
    let mut output = io::stdout();
    let ingested = load_ingested(&paths, &mut output)?;
    if ingested.is_empty() {
        writeln!(output, "{}", copy().cli.no_local_sessions_found)?;
        return Ok(());
    }

    let currency = load_currency(&paths);
    let stdin = io::stdin();
    let mut input = io::BufReader::new(stdin.lock());
    run_with_io(&mut input, &mut output, &paths, &ingested, &currency)?;
    Ok(())
}

fn load_ingested<W: Write>(paths: &ConfigPaths, output: &mut W) -> Result<Ingested> {
    writeln!(output, "{}", copy().report_cli.loading)?;
    match archive::sync_and_load(paths) {
        Ok(ingested) => Ok(ingested),
        Err(e) => {
            writeln!(
                output,
                "{}",
                template(
                    &copy().cli.archive_failed_raw_ingest,
                    &[("error", e.to_string())]
                )
            )?;
            ingest::load()
        }
    }
}

fn load_currency(paths: &ConfigPaths) -> CurrencyFormatter {
    let settings = UserConfig::load_or_create(paths).unwrap_or_default();
    let currency_table = CurrencyTable::load(paths)
        .or_else(|_| CurrencyTable::embedded())
        .expect("embedded currency rates must be valid JSON");
    currency_table.formatter(&settings.currency)
}

pub fn run_with_io<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    paths: &ConfigPaths,
    ingested: &Ingested,
    currency: &CurrencyFormatter,
) -> Result<Vec<ReportResponse>> {
    if ingested.is_empty() {
        writeln!(output, "{}", copy().cli.no_local_sessions_found)?;
        return Ok(Vec::new());
    }

    writeln!(output)?;
    writeln!(output, "{}", copy().report_cli.title)?;

    let period = prompt_period(input, output)?;
    let project_options = project_options_for(ingested, period, currency);
    let scope = prompt_project(input, output, &project_options)?;
    let formats = prompt_formats(input, output)?;
    let report_dir = prompt_folder(input, output, &reports::default_report_dir(paths))?;
    let redacted = prompt_yes_no(input, output, &copy().report_cli.select_redaction, false)?;

    write_summary(output, period, &scope, &formats, &report_dir, redacted)?;
    if !prompt_yes_no(input, output, &copy().report_cli.confirm, true)? {
        writeln!(output, "{}", copy().report_cli.cancelled)?;
        return Ok(Vec::new());
    }

    let request = ReportBatchRequest {
        formats,
        period,
        scope,
        redacted,
    };
    let responses =
        reports::write_ingested_batch_to_dir(&report_dir, &request, ingested, currency, "live")?;

    writeln!(output)?;
    writeln!(output, "{}", copy().report_cli.generated)?;
    for response in &responses {
        writeln!(
            output,
            "{}",
            template(
                &copy().report_cli.wrote,
                &[
                    ("format", response.format.label().to_string()),
                    ("path", response.path.display().to_string())
                ]
            )
        )?;
    }

    Ok(responses)
}

fn project_options_for(
    ingested: &Ingested,
    period: Period,
    currency: &CurrencyFormatter,
) -> Vec<ProjectOption> {
    let mut options = ingested.project_options(period, Tool::All, SortMode::Spend, currency);
    if options.is_empty() {
        options.push(ProjectOption::all(currency.format_money(0.0), 0));
    }
    options
}

fn prompt_period<R: BufRead, W: Write>(input: &mut R, output: &mut W) -> Result<Period> {
    let periods = Period::ALL;
    let labels: Vec<String> = periods
        .iter()
        .map(|period| period.label().to_string())
        .collect();
    let selected = prompt_numbered(input, output, &copy().report_cli.select_period, &labels, 1)?;
    Ok(periods[selected])
}

fn prompt_project<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    options: &[ProjectOption],
) -> Result<ReportScope> {
    let labels: Vec<String> = options
        .iter()
        .map(|option| {
            format!(
                "{} ({}, {} {})",
                option.label,
                option.cost,
                option.calls,
                copy().tables.calls
            )
        })
        .collect();
    let selected = prompt_numbered(input, output, &copy().report_cli.select_project, &labels, 0)?;
    Ok(options
        .get(selected)
        .map(scope_from_project_option)
        .unwrap_or(ReportScope::AllProjects))
}

fn prompt_formats<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<Vec<ReportFormat>> {
    let formats = ReportFormat::ALL;
    let labels: Vec<String> = formats
        .iter()
        .map(|format| format.label().to_string())
        .collect();
    writeln!(output)?;
    writeln!(output, "{}", copy().report_cli.select_reports)?;
    write_options(output, &labels)?;

    loop {
        let raw = prompt_line(input, output, &copy().report_cli.select_reports, "1")?;
        if raw.trim().is_empty() {
            return Ok(vec![ReportFormat::Html]);
        }
        if let Some(selected) = parse_multi_selection(&raw, formats.len()) {
            return Ok(selected.into_iter().map(|idx| formats[idx]).collect());
        }
        writeln!(output, "{}", copy().report_cli.invalid_multi)?;
    }
}

fn prompt_folder<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    default_dir: &Path,
) -> Result<PathBuf> {
    loop {
        let raw = prompt_line(
            input,
            output,
            &copy().report_cli.select_folder,
            &default_dir.display().to_string(),
        )?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(default_dir.to_path_buf());
        }
        let path = expand_home_path(trimmed);
        if path.as_os_str().is_empty() {
            writeln!(output, "{}", copy().report_cli.invalid_folder)?;
        } else {
            return Ok(path);
        }
    }
}

fn prompt_yes_no<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    default: bool,
) -> Result<bool> {
    let default_label = if default {
        copy().report_cli.yes.as_str()
    } else {
        copy().report_cli.no.as_str()
    };
    loop {
        let raw = prompt_line(input, output, label, default_label)?;
        match parse_yes_no(&raw, default) {
            Some(value) => return Ok(value),
            None => writeln!(output, "{}", copy().report_cli.invalid_yes_no)?,
        }
    }
}

fn prompt_numbered<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    options: &[String],
    default: usize,
) -> Result<usize> {
    writeln!(output)?;
    writeln!(output, "{label}")?;
    write_options(output, options)?;
    let default_label = (default + 1).to_string();

    loop {
        let raw = prompt_line(input, output, label, &default_label)?;
        if raw.trim().is_empty() {
            return Ok(default.min(options.len().saturating_sub(1)));
        }
        if let Some(idx) = parse_number_selection(&raw, options.len()) {
            return Ok(idx);
        }
        writeln!(
            output,
            "{}",
            template(
                &copy().report_cli.invalid_number,
                &[("max", options.len().to_string())]
            )
        )?;
    }
}

fn prompt_line<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    default: &str,
) -> Result<String> {
    write!(
        output,
        "{}",
        template(
            &copy().report_cli.prompt,
            &[
                ("label", label.to_string()),
                ("default", default.to_string())
            ]
        )
    )?;
    output.flush()?;

    let mut line = String::new();
    input.read_line(&mut line)?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

fn write_options<W: Write>(output: &mut W, labels: &[String]) -> Result<()> {
    for (idx, label) in labels.iter().enumerate() {
        writeln!(output, "  {}. {}", idx + 1, label)?;
    }
    Ok(())
}

fn write_summary<W: Write>(
    output: &mut W,
    period: Period,
    scope: &ReportScope,
    formats: &[ReportFormat],
    report_dir: &Path,
    redacted: bool,
) -> Result<()> {
    let format_labels = formats
        .iter()
        .map(|format| format.label())
        .collect::<Vec<_>>()
        .join(", ");
    let redaction = if redacted {
        copy().report_cli.on.as_str()
    } else {
        copy().report_cli.off.as_str()
    };

    writeln!(output)?;
    writeln!(output, "{}", copy().report_cli.summary)?;
    writeln!(output, "  {}: {}", copy().reports.period, period.label())?;
    writeln!(output, "  {}: {}", copy().reports.project, scope.label())?;
    writeln!(
        output,
        "  {}: {}",
        copy().report_cli.select_reports,
        format_labels
    )?;
    writeln!(
        output,
        "  {}: {}",
        copy().reports.folder,
        report_dir.display()
    )?;
    writeln!(output, "  {}: {}", copy().reports.redaction, redaction)?;
    Ok(())
}

fn scope_from_project_option(option: &ProjectOption) -> ReportScope {
    match &option.identity {
        Some(identity) => ReportScope::Project {
            identity: identity.clone(),
            label: option.label.clone(),
        },
        None => ReportScope::AllProjects,
    }
}

fn parse_number_selection(raw: &str, max: usize) -> Option<usize> {
    let selected = raw.trim().parse::<usize>().ok()?;
    (1..=max).contains(&selected).then_some(selected - 1)
}

fn parse_multi_selection(raw: &str, max: usize) -> Option<Vec<usize>> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("all") {
        return Some((0..max).collect());
    }

    let mut selected = BTreeSet::new();
    for part in trimmed.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        if let Some((start, end)) = part.split_once('-') {
            let start = parse_number_selection(start, max)?;
            let end = parse_number_selection(end, max)?;
            if start > end {
                return None;
            }
            selected.extend(start..=end);
        } else {
            selected.insert(parse_number_selection(part, max)?);
        }
    }

    (!selected.is_empty()).then(|| selected.into_iter().collect())
}

fn parse_yes_no(raw: &str, default: bool) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => Some(default),
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

fn expand_home_path(raw: &str) -> PathBuf {
    expand_home_path_with_home(raw, dirs::home_dir())
}

fn expand_home_path_with_home(raw: &str, home: Option<PathBuf>) -> PathBuf {
    match (raw, home) {
        ("~", Some(home)) => home,
        (value, Some(home)) if value.starts_with("~/") => home.join(&value[2..]),
        (value, _) => PathBuf::from(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use tokenuse::tools::{ParsedCall, Speed};

    fn call(project: &str) -> ParsedCall {
        ParsedCall {
            tool: tokenuse::tools::codex::config::TOOL_ID,
            model: "gpt-5".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            cached_input_tokens: 0,
            reasoning_tokens: 0,
            web_search_requests: 0,
            cost_usd: 0.25,
            tools: Vec::new(),
            bash_commands: Vec::new(),
            timestamp: Some(Utc::now() - Duration::days(1)),
            speed: Speed::Standard,
            dedup_key: "k1".into(),
            user_message: "prompt".into(),
            session_id: "s1".into(),
            project: project.into(),
        }
    }

    fn tempdir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tokenuse-report-cli-{name}-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ))
    }

    #[test]
    fn multi_selection_accepts_numbers_ranges_and_all() {
        assert_eq!(parse_multi_selection("1,3-4", 5), Some(vec![0, 2, 3]));
        assert_eq!(parse_multi_selection("all", 3), Some(vec![0, 1, 2]));
        assert_eq!(parse_multi_selection("4-2", 5), None);
        assert_eq!(parse_multi_selection("6", 5), None);
    }

    #[test]
    fn yes_no_uses_defaults_and_rejects_unknown_values() {
        assert_eq!(parse_yes_no("", true), Some(true));
        assert_eq!(parse_yes_no("", false), Some(false));
        assert_eq!(parse_yes_no("Y", false), Some(true));
        assert_eq!(parse_yes_no("no", true), Some(false));
        assert_eq!(parse_yes_no("maybe", true), None);
    }

    #[test]
    fn home_expansion_handles_tilde_and_tilde_slash_only() {
        let home = PathBuf::from("/tmp/home");

        assert_eq!(
            expand_home_path_with_home("~/reports", Some(home.clone())),
            PathBuf::from("/tmp/home/reports")
        );
        assert_eq!(
            expand_home_path_with_home("~", Some(home)),
            PathBuf::from("/tmp/home")
        );
        assert_eq!(
            expand_home_path_with_home("~other/reports", Some(PathBuf::from("/tmp/home"))),
            PathBuf::from("~other/reports")
        );
    }

    #[test]
    fn wizard_defaults_generate_html_report() {
        let dir = tempdir("defaults");
        let mut input = io::Cursor::new(format!("\n\n\n{}\n\n\n", dir.display()));
        let mut output = Vec::new();
        let ingested = Ingested {
            calls: vec![call("/tmp/project-a")],
            limits: Vec::new(),
        };

        let responses = run_with_io(
            &mut input,
            &mut output,
            &ConfigPaths::default(),
            &ingested,
            &CurrencyFormatter::usd(),
        )
        .unwrap();

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].format, ReportFormat::Html);
        assert!(responses[0].path.exists());
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Generated reports:"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wizard_retries_invalid_format_selection() {
        let dir = tempdir("retry");
        let mut input = io::Cursor::new(format!("\n\n99\n1,5-6\n{}\nn\ny\n", dir.display()));
        let mut output = Vec::new();
        let ingested = Ingested {
            calls: vec![call("/tmp/project-a")],
            limits: Vec::new(),
        };

        let responses = run_with_io(
            &mut input,
            &mut output,
            &ConfigPaths::default(),
            &ingested,
            &CurrencyFormatter::usd(),
        )
        .unwrap();

        assert_eq!(
            responses
                .iter()
                .map(|response| response.format)
                .collect::<Vec<_>>(),
            vec![ReportFormat::Html, ReportFormat::Json, ReportFormat::Xlsx]
        );
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Choose numbers, ranges, or all."));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wizard_empty_ingest_exits_without_writing_reports() {
        let dir = tempdir("empty");
        let mut input = io::Cursor::new(format!("{}\n", dir.display()));
        let mut output = Vec::new();
        let ingested = Ingested {
            calls: Vec::new(),
            limits: Vec::new(),
        };

        let responses = run_with_io(
            &mut input,
            &mut output,
            &ConfigPaths::default(),
            &ingested,
            &CurrencyFormatter::usd(),
        )
        .unwrap();

        assert!(responses.is_empty());
        assert!(!dir.exists());
        assert!(String::from_utf8(output)
            .unwrap()
            .contains(copy().cli.no_local_sessions_found.as_str()));
    }
}
