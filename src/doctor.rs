//! Read-only per-adapter diagnostics behind `tokenuse doctor`.
//!
//! For every registered tool adapter this reports the locations it probes,
//! the environment overrides in effect, how many sources discovery found,
//! and whether a bounded parse sample succeeds — so a tool that shows zero
//! (or a number that looks wrong) explains itself instead of failing
//! silently the way normal ingest deliberately does.

use std::collections::HashSet;

use serde::Serialize;

use crate::copy::{copy, template};
use crate::tools::{self, SessionSourceKind, ToolAdapter};

/// Bounded parse sample per adapter: enough to prove the parser works
/// without reading a whole large corpus.
const SESSION_SAMPLE_LIMIT: usize = 8;

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub tools: Vec<ToolReport>,
}

#[derive(Debug, Serialize)]
pub struct ToolReport {
    pub id: &'static str,
    pub name: &'static str,
    pub env: Vec<EnvOverride>,
    pub roots: Vec<RootStatus>,
    pub session_sources: usize,
    pub limit_sources: usize,
    pub sampled_sources: usize,
    pub sampled_calls: usize,
    pub sampled_limit_snapshots: usize,
    pub parse_errors: usize,
    pub verdict: Verdict,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnvOverride {
    pub name: &'static str,
    pub value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RootStatus {
    pub label: &'static str,
    pub path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Ok,
    NothingFound,
    Errors,
    DiscoveryFailed,
}

pub fn report() -> DoctorReport {
    DoctorReport {
        tools: tools::registry()
            .iter()
            .map(|adapter| diagnose(adapter.as_ref()))
            .collect(),
    }
}

fn diagnose(adapter: &dyn ToolAdapter) -> ToolReport {
    let env: Vec<EnvOverride> = adapter
        .env_overrides()
        .iter()
        .map(|name| EnvOverride {
            name,
            value: std::env::var(name).ok().filter(|value| !value.is_empty()),
        })
        .collect();
    let roots: Vec<RootStatus> = adapter
        .probe_roots()
        .into_iter()
        .map(|root| RootStatus {
            exists: root.path.exists(),
            path: root.path.display().to_string(),
            label: root.label,
        })
        .collect();

    let mut report = ToolReport {
        id: adapter.id(),
        name: adapter.display_name(),
        env,
        roots,
        session_sources: 0,
        limit_sources: 0,
        sampled_sources: 0,
        sampled_calls: 0,
        sampled_limit_snapshots: 0,
        parse_errors: 0,
        verdict: Verdict::Ok,
        detail: None,
    };

    // One adapter failing must never take down the rest of the report.
    let sources = match adapter.discover() {
        Ok(sources) => sources,
        Err(error) => {
            report.verdict = Verdict::DiscoveryFailed;
            report.detail = Some(error.to_string());
            return report;
        }
    };

    let mut session_sources: Vec<_> = sources
        .iter()
        .filter(|source| source.kind == SessionSourceKind::Session)
        .collect();
    session_sources.sort_by(|a, b| a.path.cmp(&b.path));
    let limit_sources: Vec<_> = sources
        .iter()
        .filter(|source| source.kind == SessionSourceKind::Limit)
        .collect();
    report.session_sources = session_sources.len();
    report.limit_sources = limit_sources.len();

    let mut seen = HashSet::new();
    let mut first_error: Option<String> = None;
    for source in session_sources.iter().take(SESSION_SAMPLE_LIMIT) {
        report.sampled_sources += 1;
        match adapter.parse(source, &mut seen) {
            Ok(calls) => report.sampled_calls += calls.len(),
            Err(error) => {
                report.parse_errors += 1;
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
    }
    for source in &limit_sources {
        match adapter.parse_limits(source) {
            Ok(snapshots) => report.sampled_limit_snapshots += snapshots.len(),
            Err(error) => {
                report.parse_errors += 1;
                first_error.get_or_insert_with(|| error.to_string());
            }
        }
    }

    if report.session_sources == 0 && report.limit_sources == 0 {
        report.verdict = Verdict::NothingFound;
        report.detail = Some(nothing_found_detail(&report));
    } else if report.parse_errors > 0 {
        report.verdict = Verdict::Errors;
        report.detail = first_error;
    }
    report
}

/// Explain why discovery came back empty: an override pointing at nothing is
/// the most actionable cause, then entirely missing locations (tool not
/// installed), then locations that exist but hold no usable data.
fn nothing_found_detail(report: &ToolReport) -> String {
    let doctor = &copy().doctor;
    let any_root_exists = report.roots.iter().any(|root| root.exists);
    if let Some(env) = report.env.iter().find(|env| env.value.is_some()) {
        if !any_root_exists {
            return template(
                &doctor.detail_override_missing,
                &[("name", env.name.into())],
            );
        }
    }
    if report.roots.is_empty() || !any_root_exists {
        return doctor.detail_no_roots.clone();
    }
    doctor.detail_roots_empty.clone()
}

pub fn render(report: &DoctorReport) -> String {
    let doctor = &copy().doctor;
    let mut out = String::new();
    for tool in &report.tools {
        out.push_str(&format!("{} ({})\n", tool.name, tool.id));
        for env in &tool.env {
            match &env.value {
                Some(value) => out.push_str(&format!("  env      {}={value}\n", env.name)),
                None => out.push_str(&format!(
                    "  env      {} ({})\n",
                    env.name, doctor.env_not_set
                )),
            }
        }
        for root in &tool.roots {
            let state = if root.exists {
                &doctor.root_found
            } else {
                &doctor.root_missing
            };
            out.push_str(&format!(
                "  root     {}: {} ({state})\n",
                root.label, root.path
            ));
        }
        out.push_str(&format!(
            "  sources  {}\n",
            template(
                &doctor.sources_line,
                &[
                    ("sessions", tool.session_sources.to_string()),
                    ("limits", tool.limit_sources.to_string()),
                ]
            )
        ));
        if tool.sampled_sources > 0 || tool.sampled_limit_snapshots > 0 || tool.parse_errors > 0 {
            out.push_str(&format!(
                "  sample   {}\n",
                template(
                    &doctor.sample_line,
                    &[
                        ("sources", tool.sampled_sources.to_string()),
                        ("calls", tool.sampled_calls.to_string()),
                        ("snapshots", tool.sampled_limit_snapshots.to_string()),
                        ("errors", tool.parse_errors.to_string()),
                    ]
                )
            ));
        }
        let verdict = match tool.verdict {
            Verdict::Ok => &doctor.verdict_ok,
            Verdict::NothingFound => &doctor.verdict_nothing_found,
            Verdict::Errors => &doctor.verdict_errors,
            Verdict::DiscoveryFailed => &doctor.verdict_discovery_failed,
        };
        match &tool.detail {
            Some(detail) => out.push_str(&format!("  verdict  {verdict} - {detail}\n")),
            None => out.push_str(&format!("  verdict  {verdict}\n")),
        }
        out.push('\n');
    }
    out.pop();
    out
}

pub fn run(json: bool) -> color_eyre::Result<()> {
    let report = report();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render(&report));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{LimitSnapshot, ParsedCall, ProbeRoot, SessionSource};
    use color_eyre::eyre::eyre;
    use std::path::PathBuf;

    struct StubAdapter {
        sources: Vec<SessionSource>,
        discover_fails: bool,
        parse_fails: bool,
    }

    impl ToolAdapter for StubAdapter {
        fn id(&self) -> &'static str {
            "stub"
        }
        fn display_name(&self) -> &'static str {
            "Stub"
        }
        fn discover(&self) -> color_eyre::Result<Vec<SessionSource>> {
            if self.discover_fails {
                return Err(eyre!("boom"));
            }
            Ok(self.sources.clone())
        }
        fn parse(
            &self,
            _source: &SessionSource,
            _seen: &mut HashSet<String>,
        ) -> color_eyre::Result<Vec<ParsedCall>> {
            if self.parse_fails {
                return Err(eyre!("bad json"));
            }
            Ok(vec![ParsedCall::default(), ParsedCall::default()])
        }
        fn parse_limits(&self, _source: &SessionSource) -> color_eyre::Result<Vec<LimitSnapshot>> {
            Ok(Vec::new())
        }
        fn probe_roots(&self) -> Vec<ProbeRoot> {
            vec![ProbeRoot::new(
                "sessions",
                PathBuf::from("/tokenuse-doctor-test-missing"),
            )]
        }
    }

    fn session(path: &str) -> SessionSource {
        SessionSource::session(PathBuf::from(path), "p", "stub")
    }

    #[test]
    fn healthy_adapter_reports_ok_with_sampled_calls() {
        let adapter = StubAdapter {
            sources: vec![session("/a"), session("/b")],
            discover_fails: false,
            parse_fails: false,
        };
        let report = diagnose(&adapter);
        assert_eq!(report.verdict, Verdict::Ok);
        assert_eq!(report.session_sources, 2);
        assert_eq!(report.sampled_sources, 2);
        assert_eq!(report.sampled_calls, 4);
        assert!(report.detail.is_none());
    }

    #[test]
    fn sampling_is_bounded() {
        let adapter = StubAdapter {
            sources: (0..20).map(|i| session(&format!("/s{i}"))).collect(),
            discover_fails: false,
            parse_fails: false,
        };
        let report = diagnose(&adapter);
        assert_eq!(report.session_sources, 20);
        assert_eq!(report.sampled_sources, SESSION_SAMPLE_LIMIT);
    }

    #[test]
    fn empty_discovery_explains_missing_roots() {
        let adapter = StubAdapter {
            sources: Vec::new(),
            discover_fails: false,
            parse_fails: false,
        };
        let report = diagnose(&adapter);
        assert_eq!(report.verdict, Verdict::NothingFound);
        assert!(report.detail.is_some());
    }

    #[test]
    fn parse_failures_surface_the_first_error() {
        let adapter = StubAdapter {
            sources: vec![session("/a")],
            discover_fails: false,
            parse_fails: true,
        };
        let report = diagnose(&adapter);
        assert_eq!(report.verdict, Verdict::Errors);
        assert_eq!(report.parse_errors, 1);
        assert_eq!(report.detail.as_deref(), Some("bad json"));
    }

    #[test]
    fn discovery_failure_becomes_its_own_row() {
        let adapter = StubAdapter {
            sources: Vec::new(),
            discover_fails: true,
            parse_fails: false,
        };
        let report = diagnose(&adapter);
        assert_eq!(report.verdict, Verdict::DiscoveryFailed);
        assert_eq!(report.detail.as_deref(), Some("boom"));
    }

    #[test]
    fn render_includes_every_tool_and_verdict() {
        let report = DoctorReport {
            tools: vec![diagnose(&StubAdapter {
                sources: vec![session("/a")],
                discover_fails: false,
                parse_fails: false,
            })],
        };
        let text = render(&report);
        assert!(text.contains("Stub (stub)"));
        assert!(text.contains("sessions: /tokenuse-doctor-test-missing"));
        assert!(text.contains(&copy().doctor.verdict_ok));
    }
}
