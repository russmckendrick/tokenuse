export type PageId = 'overview' | 'deep-dive' | 'usage' | 'insights' | 'audit' | 'config' | 'session';
export type PeriodId = 'today' | 'week' | 'thirty-days' | 'month' | 'all-time';
export type ToolId = 'all' | 'claude-code' | 'cursor' | 'codex' | 'copilot' | 'gemini';
export type AdviceToolId = 'codex' | 'claude-code' | 'gemini';
export type AdviceDataScopeId = 'redacted' | 'prompt_snippets';
export type AdviceItemStatusId = 'open' | 'done' | 'dismissed';
export type SortId = 'spend' | 'date' | 'tokens';
export type ReportFormatId = 'json' | 'csv' | 'svg' | 'png' | 'html' | 'pdf' | 'xlsx';

export type OptionItem<T extends string = string> = {
  value: T;
  label: string;
};

export type Summary = {
  cost: string;
  calls: string;
  sessions: string;
  cache_hit: string;
  input: string;
  output: string;
  cached: string;
  written: string;
};

export type DailyMetric = {
  day: string;
  cost: string;
  calls: number;
  value: number;
};

export type ActivityMetric = {
  label: string;
  cost: string;
  calls: number;
  value: number;
};

export type ProjectMetric = {
  name: string;
  cost: string;
  avg_per_session: string;
  sessions: number;
  tool_mix: string;
  value: number;
};

export type ProjectToolMetric = {
  project: string;
  tool: string;
  cost: string;
  calls: number;
  sessions: number;
  avg_per_session: string;
  value: number;
};

export type SessionMetric = {
  date: string;
  project: string;
  cost: string;
  calls: number;
  value: number;
};

export type ModelMetric = {
  name: string;
  cost: string;
  cache: string;
  cache_rate: string;
  calls: number;
  value: number;
};

export type CountMetric = {
  name: string;
  calls: number;
  value: number;
};

export type DashboardData = {
  summary: Summary;
  daily: DailyMetric[];
  activity_timeline: ActivityMetric[];
  projects: ProjectMetric[];
  project_tools: ProjectToolMetric[];
  sessions: SessionMetric[];
  models: ModelMetric[];
  tools: CountMetric[];
  commands: CountMetric[];
  mcp_servers: CountMetric[];
};

export type LimitMetric = {
  tool: string;
  scope: string;
  window: string;
  used: number;
  left: string;
  reset: string;
  plan: string;
};

export type RecentUsageMetric = {
  buckets: number[];
  calls: number;
  tokens: string;
  cost: string;
  last_seen: string;
};

export type RecentModelMetric = {
  name: string;
  calls: number;
  tokens: string;
  cost: string;
  value: number;
};

export type ToolLimitSection = {
  tool: string;
  limits: LimitMetric[];
  usage: RecentUsageMetric;
  models: RecentModelMetric[];
};

export type LimitsData = {
  sections: ToolLimitSection[];
};

export type ProjectOption = {
  identity: string | null;
  label: string;
  cost: string;
  calls: number;
};

export type SessionOption = {
  key: string;
  date: string;
  project: string;
  tool: string;
  cost: string;
  calls: number;
  value: number;
};

export type SessionDetail = {
  timestamp: string;
  model: string;
  cost: string;
  cache_read_rate: string;
  cache_write_rate: string;
  input_tokens: number;
  output_tokens: number;
  cache_read: number;
  cache_write: number;
  reasoning_tokens: number;
  web_search_requests: number;
  tools: string;
  bash_commands: string[];
  prompt: string;
  prompt_full: string;
};

export type SessionDetailView = {
  key: string;
  session_id: string;
  project: string;
  tool: string;
  date_range: string;
  total_cost: string;
  total_calls: number;
  total_input: string;
  total_output: string;
  total_cache_read: string;
  calls: SessionDetail[];
  note: string | null;
};

export type ConfigRow = {
  id: string;
  name: string;
  value: string;
  action: string;
  links: ConfigLink[];
};

export type ConfigLink = {
  label: string;
  url: string;
};

export type DesktopSettingsState = {
  open_at_login: boolean;
  show_dock_or_taskbar_icon: boolean;
};

export type DesktopUpdateState = {
  supported: boolean;
};

export type DesktopUpdateMetadata = {
  version: string;
  currentVersion: string;
};

export type DesktopUpdateDownloadEvent =
  | { event: 'started'; data: { contentLength: number | null } }
  | { event: 'progress'; data: { chunkLength: number } }
  | { event: 'finished' };

export type ProjectState = {
  identity: string | null;
  label: string;
};

export type InsightsScopeView = {
  kind: 'all' | 'project' | 'project_model' | 'tool' | 'session';
  label: string | null;
  project: string | null;
  session: string | null;
  tool: string | null;
  model: string | null;
};

export type RecommendationView = {
  id: string;
  rule_id: string;
  category: string;
  category_label: string;
  severity: 'risk' | 'warn' | 'info';
  severity_label: string;
  title: string;
  body: string;
  assumption: string | null;
  savings: string | null;
  savings_amount_usd: number | null;
  scope: InsightsScopeView;
  silenced_reason: string | null;
};

export type InsightsCategoryCount = {
  id: string;
  label: string;
  count: number;
};

export type InsightsSeverityCount = {
  id: string;
  label: string;
  count: number;
};

export type InsightsSummary = {
  total_est_savings_usd: number;
  total_est_savings: string;
  by_category: InsightsCategoryCount[];
  by_severity: InsightsSeverityCount[];
};

export type InsightsView = {
  generated_at: string;
  baseline_window_days: number;
  summary: InsightsSummary;
  recommendations: RecommendationView[];
};

export type AdviceItemView = {
  id: number;
  run_id: number;
  title: string;
  body: string;
  category: string;
  severity: string;
  confidence: number;
  impact: string;
  estimated_savings_usd: number | null;
  evidence: string[];
  next_step: string;
  status: AdviceItemStatusId;
  notes: string | null;
};

export type AdviceRunView = {
  id: number;
  created_at: string;
  tool: AdviceToolId;
  tool_label: string;
  data_scope: AdviceDataScopeId;
  status: 'succeeded' | 'failed';
  summary: string | null;
  raw_output: string;
  error: string | null;
  items: AdviceItemView[];
};

export type AdviceHistory = {
  runs: AdviceRunView[];
};

export type AuditSection = 'security' | 'efficiency' | 'context' | 'readiness';
export type AuditSeverity = 'risk' | 'warning' | 'info';

export type AuditSummary = {
  total_findings: number;
  security_findings: number;
  efficiency_findings: number;
  context_findings: number;
  readiness_findings: number;
  risk_findings: number;
  warning_findings: number;
  info_findings: number;
};

export type AuditToolSummary = {
  id: string;
  label: string;
  present: boolean;
  config_paths: string[];
  mcp_servers: string[];
  hooks_count: number;
  knowledge_files: number;
  scoped_assets: number;
  dangerous_alias_detected: boolean;
};

export type AuditBehavior = {
  recent_sessions_inspected: number;
  clear_uses: number | null;
  compact_uses: number | null;
  subagent_calls: number | null;
  plan_mode_uses: number | null;
  skill_invocations: number | null;
  longest_session_turns_without_reset: number | null;
  avg_user_turn_chars: number | null;
  correction_turn_ratio: number | null;
};

export type AuditKnowledgeFlags = {
  mentions_testing: boolean;
  mentions_security: boolean;
  mentions_secrets: boolean;
  has_wrong_right_patterns: boolean;
  has_command_table: boolean;
  has_dont_section: boolean;
  has_external_links: boolean;
  style_keywords: string[];
  imports_other_files: boolean;
  imported_paths: string[];
};

export type AuditKnowledgeFile = {
  path: string;
  exists: boolean;
  size_bytes: number;
  line_count: number;
  content_preview: string;
  content_truncated: boolean;
  feature_flags: AuditKnowledgeFlags;
};

export type AuditRankedItem = {
  name: string;
  calls: number;
  sessions: number;
  cost_usd: number;
  cost_label: string;
  tokens: number;
};

export type AuditUsageSummary = {
  available: boolean;
  window_label: string;
  calls: number;
  sessions: number;
  cost_usd: number;
  cost_label: string;
  input_tokens: number;
  output_tokens: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
  total_tokens: number;
  cache_read_ratio: number | null;
  cache_hit_ratio: number | null;
  top_tools: AuditRankedItem[];
  top_models: AuditRankedItem[];
  top_projects: AuditRankedItem[];
};

export type AuditProjectCoverageEntry = {
  label: string;
  path: string;
  calls: number;
  sessions: number;
  agent_files: string[];
  has_ci: boolean;
  has_manifest: boolean;
  checked: boolean;
  skipped_reason: string | null;
};

export type AuditProjectCoverage = {
  available: boolean;
  known_project_roots: number;
  checked_project_roots: number;
  roots_with_agent_instructions: number;
  roots_with_ci: number;
  roots_with_manifests: number;
  skipped_project_roots: number;
  omitted_project_roots: number;
  entries: AuditProjectCoverageEntry[];
};

export type AuditActivitySignals = {
  available: boolean;
  tool_call_uses: number;
  shell_command_uses: number;
  mcp_tool_uses: number;
  distinct_tools_used: number;
  distinct_models_used: number;
  high_cost_projects: string[];
  high_cost_sessions: string[];
  repeated_model_patterns: string[];
  repeated_tool_patterns: string[];
};

export type AuditFinding = {
  id: string;
  section: AuditSection;
  severity: AuditSeverity;
  title: string;
  body: string;
  evidence: string[];
  source_paths: string[];
};

export type AuditSnapshot = {
  schema_version: string;
  scanner_version: string;
  captured_at: string;
  root: string | null;
  primary_tool_guess: string | null;
  redaction: { enabled: boolean; secrets_redacted: boolean; home_paths_folded: boolean };
  tools: AuditToolSummary[];
  usage_summary: AuditUsageSummary;
  recent_usage: AuditUsageSummary;
  project_coverage: AuditProjectCoverage;
  activity_signals: AuditActivitySignals;
  behavior: AuditBehavior;
  summary: AuditSummary;
  findings: AuditFinding[];
  knowledge_files: AuditKnowledgeFile[];
};

export type ShortcutHint = {
  keys: string;
  label: string;
  action: string;
};

export type CopyHintGroup = {
  title: string;
  items: ShortcutHint[];
};

export type CopyDeck = {
  brand: Record<string, string>;
  nav: Record<string, string>;
  periods: Record<string, string>;
  sorts: Record<string, string>;
  tools: Record<string, string>;
  metrics: Record<string, string>;
  filters: Record<string, string>;
  panels: Record<string, string>;
  tables: Record<string, string>;
  timeline: Record<string, string>;
  usage: Record<string, string>;
  config: {
    rows: Record<string, { name: string; action: string }>;
    values: Record<string, string>;
    paths: Record<string, string>;
  };
  session: Record<string, string>;
  modals: Record<string, string>;
  actions: Record<string, string>;
  desktop: Record<string, string>;
  updates: Record<string, string>;
  tray: Record<string, string>;
  empty: Record<string, string>;
  export: Record<string, unknown>;
  reports: Record<string, string>;
  cli: Record<string, string>;
  insights: {
    title: string;
    subtitle: string;
    signals_title: string;
    advice_title: string;
    advice_empty: string;
    advice_failed: string;
    advice_scope_title: string;
    advice_scope_redacted: string;
    advice_scope_redacted_detail: string;
    advice_scope_snippets: string;
    advice_scope_snippets_detail: string;
    advice_generate_button: string;
    advice_scope_running: string;
    advice_latest: string;
    advice_run: string;
    advice_confidence: string;
    advice_next_step: string;
    advice_evidence: string;
    dashboard_label: string;
    screen_nav_label: string;
    screen_overview: string;
    screen_actions: string;
    screen_signals: string;
    screen_runs: string;
    overview_digest_title: string;
    overview_digest_empty: string;
    top_actions_title: string;
    top_actions_detail: string;
    actions_title: string;
    actions_subtitle: string;
    actions_empty_title: string;
    actions_empty_detail: string;
    signals_subtitle: string;
    signals_empty_title: string;
    signals_empty_detail: string;
    runs_title: string;
    runs_subtitle: string;
    local_badge: string;
    scope_badge: string;
    latest_run_title: string;
    estimated_savings: string;
    source_advice: string;
    source_signal: string;
    status_signal: string;
    status_open: string;
    status_done: string;
    status_dismissed: string;
    status_succeeded: string;
    status_failed: string;
    scope_all: string;
    filter_title: string;
    filter_all: string;
    filter_open: string;
    filter_done: string;
    filter_dismissed: string;
    filter_advice: string;
    filter_signals: string;
    detail_open: string;
    detail_back: string;
    detail_observation: string;
    detail_impact: string;
    detail_next_step: string;
    detail_guardrails: string;
    detail_evidence: string;
    detail_scope: string;
    detail_status: string;
    detail_savings: string;
    detail_rule: string;
    detail_empty_title: string;
    detail_empty_detail: string;
    latest_issue_title: string;
    no_failed_runs_title: string;
    no_failed_runs_detail: string;
    signal_map_title: string;
    run_history_title: string;
    data_context_title: string;
    generated_at_label: string;
    generated_at_value: string;
    baseline_window_label: string;
    baseline_window_value: string;
    archive_source_label: string;
    advice_tool_label: string;
    run_items: string;
    kpi_savings: string;
    kpi_risks: string;
    kpi_warns: string;
    kpi_infos: string;
    empty: string;
    categories: Record<string, string>;
    severity: Record<string, string>;
    savings: { per_week: string; per_month: string; one_off: string };
    silenced: Record<string, string>;
    rules: Record<string, { title: string; body: string; assumption?: string | null }>;
  };
  audit: {
    title: string;
    subtitle: string;
    refresh: string;
    captured_at: string;
    not_captured: string;
    scanner_version: string;
    primary_tool: string;
    redaction: string;
    tools_title: string;
    findings_title: string;
    knowledge_title: string;
    behavior_title: string;
    project_title: string;
    coverage_title: string;
    all_time: string;
    recent_7d: string;
    no_archive_data: string;
    no_recent_calls: string;
    no_readable_project_roots: string;
    not_measured: string;
    no_findings: string;
    sections: Record<AuditSection, string>;
    severity: Record<AuditSeverity, string>;
  };
  keymap: {
    actions: Record<string, string>;
    help: CopyHintGroup[];
    footers: Record<string, ShortcutHint[]>;
  };
  status: Record<string, string>;
};

export type ShortcutInput = {
  key: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
};

export type DesktopSnapshot = {
  copy: CopyDeck;
  version: string;
  source: 'live' | 'sample';
  status: string | null;
  status_tone: 'info' | 'busy' | 'success' | 'warning' | 'error';
  page: PageId;
  period: PeriodId;
  periods: OptionItem<PeriodId>[];
  tool: ToolId;
  tools: OptionItem<ToolId>[];
  sort: SortId;
  sorts: OptionItem<SortId>[];
  project: ProjectState;
  dashboard: DashboardData;
  usage: LimitsData;
  insights: InsightsView;
  advice: AdviceHistory;
  audit: AuditSnapshot;
  advice_running: boolean;
  advice_tool: AdviceToolId;
  advice_tool_options: OptionItem<AdviceToolId>[];
  projects: ProjectOption[];
  report_projects: ProjectOption[];
  sessions: SessionOption[];
  session: SessionDetailView | null;
  config_rows: ConfigRow[];
  currencies: string[];
  currency: string;
  desktop_settings: DesktopSettingsState;
  desktop_updates: DesktopUpdateState;
  report_dir: string;
  report_formats: OptionItem<ReportFormatId>[];
  shortcut_footer: ShortcutHint[];
  subscription_cookies: SubscriptionCookieState;
};

export type SubscriptionCookieState = {
  supported: boolean;
  claude_set: boolean;
  codex_set: boolean;
};

export type TraySnapshot = {
  copy: CopyDeck;
  version: string;
  status: string | null;
  currency: string;
  dashboard: DashboardData;
  usage: LimitsData;
};

export type ReportResponse = {
  path: string;
  snapshot: DesktopSnapshot;
};

export type ShortcutResponse = {
  handled: boolean;
  effect: 'open_project_picker' | 'open_session_picker' | 'open_export_picker' | 'close_modal' | 'close_call_detail' | null;
  snapshot: DesktopSnapshot;
};
