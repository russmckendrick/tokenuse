//! Period-aware relationship graph derived from normalized usage calls.
//!
//! The graph is intentionally an analytical projection rather than a raw
//! call graph: projects, desktop tools, canonical models, core tools, and MCP
//! servers are aggregated locally, then deterministically bounded for a
//! legible desktop visualization.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Duration, Local, Utc};
use serde::Serialize;

use crate::app::{Period, ProjectFilter, Tool};
use crate::copy::copy;
use crate::currency::CurrencyFormatter;
use crate::ingest::projects::{project_identity, project_label, project_label_lookup};
use crate::ingest::{in_period, matches_project, matches_tool, session_key};
use crate::tools::ParsedCall;

const MAX_PROJECTS: usize = 30;
const MAX_MODELS: usize = 24;
const MAX_CORE_TOOLS: usize = 12;
const MAX_MCP_SERVERS: usize = 12;
const MAX_MODELS_PER_PROJECT: usize = 6;
const MAX_CAPABILITIES_PER_PROJECT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphMetric {
    Calls,
    Spend,
    Tokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Project,
    Tool,
    Model,
    CoreTool,
    McpServer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRelation {
    ProjectTool,
    ProjectModel,
    ToolModel,
    ProjectCoreTool,
    ToolCoreTool,
    ProjectMcpServer,
    ToolMcpServer,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphStats {
    pub calls: u64,
    pub sessions: u64,
    pub tokens: u64,
    pub cost: String,
    pub cost_value: f64,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphNodeKind,
    pub label: String,
    pub entity_id: String,
    pub provider: String,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: GraphRelation,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphMeta {
    pub total_nodes: usize,
    pub shown_nodes: usize,
    pub total_edges: usize,
    pub shown_edges: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub meta: GraphMeta,
}

#[derive(Debug, Clone, Default)]
struct StatsAcc {
    calls: u64,
    sessions: HashSet<String>,
    tokens: f64,
    cost: f64,
    last: Option<DateTime<Utc>>,
}

impl StatsAcc {
    fn add(&mut self, call: &ParsedCall, share: f64) {
        self.calls = self.calls.saturating_add(1);
        if let Some(key) = session_key(call) {
            self.sessions.insert(key);
        }
        self.tokens += activity_tokens(call) as f64 * share;
        self.cost += call.cost_usd * share;
        if call.timestamp > self.last {
            self.last = call.timestamp;
        }
    }

    fn metric_value(&self, metric: GraphMetric) -> f64 {
        match metric {
            GraphMetric::Calls => self.calls as f64,
            GraphMetric::Spend => self.cost,
            GraphMetric::Tokens => self.tokens,
        }
    }

    fn finish(&self, currency: &CurrencyFormatter) -> GraphStats {
        GraphStats {
            calls: self.calls,
            sessions: self.sessions.len() as u64,
            tokens: self.tokens.round().max(0.0) as u64,
            cost: currency.format_money(self.cost),
            cost_value: self.cost,
            last_activity: self
                .last
                .map(|timestamp| {
                    timestamp
                        .with_timezone(&Local)
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_else(|| "-".into()),
        }
    }
}

#[derive(Debug, Clone)]
struct NodeAcc {
    kind: GraphNodeKind,
    label: String,
    entity_id: String,
    provider: String,
    stats: StatsAcc,
}

impl NodeAcc {
    fn new(
        kind: GraphNodeKind,
        label: impl Into<String>,
        entity_id: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            label: label.into(),
            entity_id: entity_id.into(),
            provider: provider.into(),
            stats: StatsAcc::default(),
        }
    }
}

type EdgeKey = (GraphRelation, String, String);

pub fn graph_data(
    calls: &[ParsedCall],
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    metric: GraphMetric,
    currency: &CurrencyFormatter,
) -> GraphData {
    graph_data_at(
        calls,
        period,
        tool,
        project_filter,
        metric,
        currency,
        Local::now(),
    )
}

fn graph_data_at(
    calls: &[ParsedCall],
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    metric: GraphMetric,
    currency: &CurrencyFormatter,
    now: DateTime<Local>,
) -> GraphData {
    let filtered: Vec<&ParsedCall> = calls
        .iter()
        .filter(|call| {
            matches_tool(call, tool)
                && matches_project(call, project_filter)
                && in_period(call, period, now)
        })
        .collect();
    if filtered.is_empty() {
        return GraphData::default();
    }

    let labels = project_label_lookup(filtered.iter().map(|call| &call.project));
    let mut nodes: BTreeMap<String, NodeAcc> = BTreeMap::new();
    let mut edges: BTreeMap<EdgeKey, StatsAcc> = BTreeMap::new();

    for call in filtered {
        let project_identity = project_identity(&call.project);
        let project_id = node_id(GraphNodeKind::Project, &project_identity);
        let tool_id = node_id(GraphNodeKind::Tool, call.tool);
        let model = crate::models::resolve(call.tool, &call.model);
        let model_id = node_id(GraphNodeKind::Model, &model.canonical_id);

        add_node(
            &mut nodes,
            &project_id,
            GraphNodeKind::Project,
            project_label(&labels, &project_identity),
            &project_identity,
            "",
            call,
            1.0,
        );
        add_node(
            &mut nodes,
            &tool_id,
            GraphNodeKind::Tool,
            desktop_tool_label(call.tool),
            call.tool,
            call.tool,
            call,
            1.0,
        );
        add_node(
            &mut nodes,
            &model_id,
            GraphNodeKind::Model,
            model.display,
            &model.canonical_id,
            model.provider.id(),
            call,
            1.0,
        );

        add_edge(
            &mut edges,
            GraphRelation::ProjectTool,
            &project_id,
            &tool_id,
            call,
            1.0,
        );
        add_edge(
            &mut edges,
            GraphRelation::ProjectModel,
            &project_id,
            &model_id,
            call,
            1.0,
        );
        add_edge(
            &mut edges,
            GraphRelation::ToolModel,
            &tool_id,
            &model_id,
            call,
            1.0,
        );

        let capabilities: Vec<(GraphNodeKind, String)> = call
            .tools
            .iter()
            .filter_map(|name| capability(name))
            .collect();
        let share = 1.0 / capabilities.len().max(1) as f64;
        for (kind, name) in capabilities {
            let capability_id = node_id(kind, &name);
            add_node(
                &mut nodes,
                &capability_id,
                kind,
                &name,
                &name,
                "",
                call,
                share,
            );
            let (project_relation, tool_relation) = match kind {
                GraphNodeKind::CoreTool => {
                    (GraphRelation::ProjectCoreTool, GraphRelation::ToolCoreTool)
                }
                GraphNodeKind::McpServer => (
                    GraphRelation::ProjectMcpServer,
                    GraphRelation::ToolMcpServer,
                ),
                _ => continue,
            };
            add_edge(
                &mut edges,
                project_relation,
                &project_id,
                &capability_id,
                call,
                share,
            );
            add_edge(
                &mut edges,
                tool_relation,
                &tool_id,
                &capability_id,
                call,
                share,
            );
        }
    }

    bound_graph(nodes, edges, metric, currency)
}

#[allow(clippy::too_many_arguments)]
fn add_node(
    nodes: &mut BTreeMap<String, NodeAcc>,
    id: &str,
    kind: GraphNodeKind,
    label: impl Into<String>,
    entity_id: &str,
    provider: &str,
    call: &ParsedCall,
    share: f64,
) {
    let entry = nodes
        .entry(id.to_string())
        .or_insert_with(|| NodeAcc::new(kind, label, entity_id.to_string(), provider.to_string()));
    entry.stats.add(call, share);
}

fn add_edge(
    edges: &mut BTreeMap<EdgeKey, StatsAcc>,
    relation: GraphRelation,
    source: &str,
    target: &str,
    call: &ParsedCall,
    share: f64,
) {
    edges
        .entry((relation, source.to_string(), target.to_string()))
        .or_default()
        .add(call, share);
}

fn bound_graph(
    nodes: BTreeMap<String, NodeAcc>,
    edges: BTreeMap<EdgeKey, StatsAcc>,
    metric: GraphMetric,
    currency: &CurrencyFormatter,
) -> GraphData {
    let total_nodes = nodes.len();
    let total_edges = edges.len();
    let mut retained = HashSet::new();

    for kind in [
        GraphNodeKind::Project,
        GraphNodeKind::Tool,
        GraphNodeKind::Model,
        GraphNodeKind::CoreTool,
        GraphNodeKind::McpServer,
    ] {
        let limit = match kind {
            GraphNodeKind::Project => MAX_PROJECTS,
            GraphNodeKind::Tool => usize::MAX,
            GraphNodeKind::Model => MAX_MODELS,
            GraphNodeKind::CoreTool => MAX_CORE_TOOLS,
            GraphNodeKind::McpServer => MAX_MCP_SERVERS,
        };
        let mut rows: Vec<(&String, &NodeAcc)> =
            nodes.iter().filter(|(_, node)| node.kind == kind).collect();
        rows.sort_by(|a, b| compare_stats(&a.1.stats, &b.1.stats, &a.1.label, &b.1.label, metric));
        retained.extend(rows.into_iter().take(limit).map(|(id, _)| id.clone()));
    }

    let mut candidate_edges: Vec<(&EdgeKey, &StatsAcc)> = edges
        .iter()
        .filter(|((_, source, target), _)| retained.contains(source) && retained.contains(target))
        .collect();
    candidate_edges.sort_by(|a, b| {
        a.0 .0
            .cmp(&b.0 .0)
            .then_with(|| a.0 .1.cmp(&b.0 .1))
            .then_with(|| compare_stats(a.1, b.1, &a.0 .2, &b.0 .2, metric))
    });

    let mut per_project: HashMap<(GraphRelation, &str), usize> = HashMap::new();
    let mut kept_edges = Vec::new();
    for (key, stats) in candidate_edges {
        let limit = match key.0 {
            GraphRelation::ProjectModel => Some(MAX_MODELS_PER_PROJECT),
            GraphRelation::ProjectCoreTool | GraphRelation::ProjectMcpServer => {
                Some(MAX_CAPABILITIES_PER_PROJECT)
            }
            _ => None,
        };
        if let Some(limit) = limit {
            let count = per_project.entry((key.0, key.1.as_str())).or_default();
            if *count >= limit {
                continue;
            }
            *count += 1;
        }
        kept_edges.push((key, stats));
    }

    let connected: HashSet<&str> = kept_edges
        .iter()
        .flat_map(|(key, _)| [key.1.as_str(), key.2.as_str()])
        .collect();
    let mut result_nodes: Vec<GraphNode> = nodes
        .into_iter()
        .filter(|(id, _)| retained.contains(id) && connected.contains(id.as_str()))
        .map(|(id, node)| GraphNode {
            id,
            kind: node.kind,
            label: node.label,
            entity_id: node.entity_id,
            provider: node.provider,
            stats: node.stats.finish(currency),
        })
        .collect();
    result_nodes.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| compare_finished_stats(&a.stats, &b.stats, &a.label, &b.label, metric))
    });

    let valid_ids: HashSet<&str> = result_nodes.iter().map(|node| node.id.as_str()).collect();
    let mut result_edges: Vec<GraphEdge> = kept_edges
        .into_iter()
        .filter(|(key, _)| valid_ids.contains(key.1.as_str()) && valid_ids.contains(key.2.as_str()))
        .map(|(key, stats)| GraphEdge {
            id: format!("{:?}:{}:{}", key.0, key.1, key.2),
            source: key.1.clone(),
            target: key.2.clone(),
            relation: key.0,
            stats: stats.finish(currency),
        })
        .collect();
    result_edges.sort_by(|a, b| {
        a.relation
            .cmp(&b.relation)
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| compare_finished_stats(&a.stats, &b.stats, &a.target, &b.target, metric))
    });

    GraphData {
        meta: GraphMeta {
            total_nodes,
            shown_nodes: result_nodes.len(),
            total_edges,
            shown_edges: result_edges.len(),
        },
        nodes: result_nodes,
        edges: result_edges,
    }
}

fn compare_stats(
    a: &StatsAcc,
    b: &StatsAcc,
    a_label: &str,
    b_label: &str,
    metric: GraphMetric,
) -> Ordering {
    b.metric_value(metric)
        .partial_cmp(&a.metric_value(metric))
        .unwrap_or(Ordering::Equal)
        .then_with(|| a_label.cmp(b_label))
}

fn compare_finished_stats(
    a: &GraphStats,
    b: &GraphStats,
    a_label: &str,
    b_label: &str,
    metric: GraphMetric,
) -> Ordering {
    let value = |stats: &GraphStats| match metric {
        GraphMetric::Calls => stats.calls as f64,
        GraphMetric::Spend => stats.cost_value,
        GraphMetric::Tokens => stats.tokens as f64,
    };
    value(b)
        .partial_cmp(&value(a))
        .unwrap_or(Ordering::Equal)
        .then_with(|| a_label.cmp(b_label))
}

fn node_id(kind: GraphNodeKind, value: &str) -> String {
    let prefix = match kind {
        GraphNodeKind::Project => "project",
        GraphNodeKind::Tool => "tool",
        GraphNodeKind::Model => "model",
        GraphNodeKind::CoreTool => "core",
        GraphNodeKind::McpServer => "mcp",
    };
    format!("{prefix}:{value}")
}

fn desktop_tool_label(tool: &str) -> String {
    let tools = &copy().tools;
    match tool {
        "claude-code" => tools.claude_code.clone(),
        "cursor" => tools.cursor.clone(),
        "codex" => tools.codex.clone(),
        "copilot" => tools.copilot.clone(),
        "gemini" => tools.gemini.clone(),
        other => other.to_string(),
    }
}

fn capability(name: &str) -> Option<(GraphNodeKind, String)> {
    if let Some(rest) = name.strip_prefix("mcp__") {
        let server = rest.split("__").next().unwrap_or(rest).trim();
        if server.is_empty() {
            None
        } else {
            Some((GraphNodeKind::McpServer, server.to_string()))
        }
    } else if name.trim().is_empty() {
        None
    } else {
        Some((GraphNodeKind::CoreTool, name.trim().to_string()))
    }
}

fn activity_tokens(call: &ParsedCall) -> u64 {
    call.input_tokens
        .saturating_add(call.output_tokens)
        .saturating_add(call.cache_creation_input_tokens)
        .saturating_add(call.cache_read_input_tokens)
}

/// Privacy-safe sample observations rendered through the same aggregation and
/// pruning path as live calls.
pub fn sample_graph_data(
    period: Period,
    tool: Tool,
    project_filter: &ProjectFilter,
    metric: GraphMetric,
    currency: &CurrencyFormatter,
) -> GraphData {
    let now = Utc::now();
    let seeds = [
        (
            "acme/atlas-dashboard",
            "claude-code",
            "claude-opus-4-7",
            8,
            2,
            0.18,
            &["Read", "Edit", "mcp__github__search"] as &[&str],
        ),
        (
            "acme/atlas-dashboard",
            "codex",
            "gpt-5",
            6,
            5,
            0.12,
            &["exec_command", "apply_patch", "mcp__github__pull_request"],
        ),
        (
            "northstar/cli",
            "codex",
            "gpt-5",
            7,
            12,
            0.09,
            &["exec_command", "apply_patch", "mcp__github__issues"],
        ),
        (
            "northstar/cli",
            "gemini",
            "gemini-2.5-pro",
            3,
            36,
            0.05,
            &["Read", "Write"],
        ),
        (
            "orbit/mobile-app",
            "cursor",
            "claude-sonnet-4-5",
            6,
            18,
            0.08,
            &["Read", "Edit", "mcp__figma__get_file"],
        ),
        (
            "orbit/mobile-app",
            "copilot",
            "anthropic-auto",
            4,
            54,
            0.04,
            &["completion"],
        ),
        (
            "brightlane/docs",
            "claude-code",
            "claude-sonnet-4-5",
            5,
            20,
            0.06,
            &["Read", "Write", "mcp__notion__search"],
        ),
        (
            "brightlane/docs",
            "copilot",
            "openai-auto",
            4,
            80,
            0.03,
            &["completion"],
        ),
        (
            "harbor/data-tools",
            "gemini",
            "gemini-2.5-flash",
            5,
            8,
            0.035,
            &["Read", "exec", "mcp__github__search"],
        ),
        (
            "harbor/data-tools",
            "claude-code",
            "claude-opus-4-7",
            3,
            160,
            0.14,
            &["Read", "Bash", "mcp__github__issues"],
        ),
    ];
    let mut calls = Vec::new();
    for (project, tool_id, model, count, age_hours, cost, tools) in seeds {
        for index in 0..count {
            calls.push(ParsedCall {
                tool: tool_id,
                model: model.into(),
                input_tokens: 2_400 + index * 170,
                output_tokens: 620 + index * 45,
                cache_read_input_tokens: 1_100,
                cost_usd: cost,
                tools: tools.iter().map(|value| (*value).to_string()).collect(),
                timestamp: Some(now - Duration::hours(age_hours + index as i64)),
                dedup_key: format!("sample-graph-{project}-{tool_id}-{index}"),
                session_id: format!("sample-graph-{project}-{}", index / 3),
                project: project.into(),
                ..ParsedCall::default()
            });
        }
    }
    graph_data(&calls, period, tool, project_filter, metric, currency)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(project: &str, tool: &'static str, model: &str, session: &str) -> ParsedCall {
        ParsedCall {
            tool,
            model: model.into(),
            project: project.into(),
            session_id: session.into(),
            dedup_key: format!("{project}-{tool}-{model}-{session}"),
            timestamp: Some(Utc::now()),
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: 1.0,
            ..ParsedCall::default()
        }
    }

    #[test]
    fn graph_folds_models_and_deduplicates_sessions() {
        let mut first = call("work/alpha", "claude-code", "claude-opus-4-7", "one");
        first.tools = vec!["Read".into(), "mcp__github__search".into()];
        let second = call(
            "work/alpha",
            "claude-code",
            "claude-opus-4-7-20260101",
            "one",
        );
        let data = graph_data(
            &[first, second],
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            GraphMetric::Calls,
            &CurrencyFormatter::usd(),
        );
        let model = data
            .nodes
            .iter()
            .find(|node| node.kind == GraphNodeKind::Model)
            .expect("model node");
        assert_eq!(model.entity_id, "claude-opus-4-7");
        assert_eq!(model.stats.calls, 2);
        assert_eq!(model.stats.sessions, 1);
        assert!(data.edges.iter().all(|edge| {
            data.nodes.iter().any(|node| node.id == edge.source)
                && data.nodes.iter().any(|node| node.id == edge.target)
        }));
    }

    #[test]
    fn graph_scopes_tool_and_project() {
        let calls = vec![
            call("work/alpha", "claude-code", "claude-opus-4-7", "one"),
            call("work/beta", "codex", "gpt-5", "two"),
        ];
        let filter = ProjectFilter::Selected {
            identity: "work/beta".into(),
            label: "beta".into(),
        };
        let data = graph_data(
            &calls,
            Period::AllTime,
            Tool::Codex,
            &filter,
            GraphMetric::Spend,
            &CurrencyFormatter::usd(),
        );
        assert!(data.nodes.iter().all(|node| !node.id.contains("alpha")));
        assert!(data.nodes.iter().any(|node| node.id == "tool:codex"));
    }

    #[test]
    fn capability_cost_and_tokens_are_shared_without_losing_occurrences() {
        let mut row = call("work/alpha", "codex", "gpt-5", "one");
        row.tools = vec!["Read".into(), "mcp__github__search".into()];
        let data = graph_data(
            &[row],
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            GraphMetric::Tokens,
            &CurrencyFormatter::usd(),
        );
        let capabilities: Vec<&GraphNode> = data
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.kind,
                    GraphNodeKind::CoreTool | GraphNodeKind::McpServer
                )
            })
            .collect();
        assert_eq!(capabilities.len(), 2);
        assert!(capabilities.iter().all(|node| node.stats.calls == 1));
        assert!(
            (capabilities
                .iter()
                .map(|node| node.stats.cost_value)
                .sum::<f64>()
                - 1.0)
                .abs()
                < 0.0001
        );
        assert_eq!(
            capabilities
                .iter()
                .map(|node| node.stats.tokens)
                .sum::<u64>(),
            150
        );
    }

    #[test]
    fn graph_caps_projects_and_reports_omissions() {
        let calls: Vec<ParsedCall> = (0..40)
            .map(|index| call(&format!("work/project-{index:02}"), "codex", "gpt-5", "one"))
            .collect();
        let data = graph_data(
            &calls,
            Period::AllTime,
            Tool::All,
            &ProjectFilter::All,
            GraphMetric::Calls,
            &CurrencyFormatter::usd(),
        );
        assert_eq!(
            data.nodes
                .iter()
                .filter(|node| node.kind == GraphNodeKind::Project)
                .count(),
            MAX_PROJECTS
        );
        assert!(data.meta.total_nodes > data.meta.shown_nodes);
        assert!(data.meta.total_edges > data.meta.shown_edges);
    }

    #[test]
    fn sample_graph_is_nonempty_and_privacy_safe() {
        let data = sample_graph_data(
            Period::Week,
            Tool::All,
            &ProjectFilter::All,
            GraphMetric::Calls,
            &CurrencyFormatter::usd(),
        );
        let encoded = serde_json::to_string(&data)
            .expect("serializes")
            .to_lowercase();
        assert!(!data.nodes.is_empty());
        for banned in ["/users/", "russ", "mckendrick"] {
            assert!(!encoded.contains(banned));
        }
    }
}
