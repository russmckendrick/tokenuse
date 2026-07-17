export type PeriodId = 'today' | 'week' | 'thirty-days' | 'month' | 'all-time';
export type ToolId = 'all' | 'claude-code' | 'cursor' | 'codex' | 'copilot' | 'gemini';
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
  /** Drill-down key for the session view; empty when the row is display-only. */
  key: string;
  date: string;
  project: string;
  cost: string;
  calls: number;
  value: number;
};

export type ModelMetric = {
  name: string;
  provider: string;
  provider_label: string;
  family: string;
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

export type ModelCatalogEntry = {
  canonical_id: string;
  name: string;
  provider: string;
  provider_label: string;
  family: string;
  cost: string;
  calls: number;
  tokens: string;
  cache_hit: string;
  value: number;
  per_tool: ModelToolBreakdown[];
};

export type ModelToolBreakdown = {
  tool: string;
  tool_label: string;
  cost: string;
  calls: number;
  value: number;
};

export type AnalyticsData = {
  daily_by_tool: StackedDayMetric[];
  hour_day: number[][];
  provider_share: ShareMetric[];
  tool_share: ShareMetric[];
};

export type CoachData = {
  overall: CoachOverall;
  practice_groups: PracticeGroupScore[];
  findings: CoachFinding[];
  /** Advisory configuration findings; never move the grade. */
  setup: CoachSetupFinding[];
  flow: FlowSummary;
  pace: PaceSummary;
  output: OutputSummary;
  timeline_grid: TimelineGridDay[];
  projects: CoachProjectActivity[];
};

export type CoachSetupFinding = {
  id: string;
  title: string;
  detail: string;
  savings_tokens: number;
  savings_label: string;
};

export type CoachProjectActivity = {
  /** Dashboard-consistent short label (joins DashboardData.projects). */
  name: string;
  /** Formatted block-based active time ("14.2h", "45m"). */
  active_hours: string;
  turns: number;
  /** Code-output languages by LoC, descending - the observed tech stack. */
  languages: CountMetric[];
  /** Most-edited file paths, descending by edit count. */
  hot_files: string[];
  /** Copy id for the weekday/weekend mix; empty without timestamps. */
  days_id: string;
  /** Copy id for the dominant daypart; empty when none dominates. */
  time_id: string;
};

export type TimelineGridDay = {
  day: string;
  turns: number;
  /** False for context days outside the selected period (rendered dimmed). */
  in_period: boolean;
};

export type CoachOverall = {
  score: number;
  grade_id: string;
};

export type PracticeGroupScore = {
  id: string;
  score: number;
  grade_id: string;
  wow: string;
  mom: string;
  trend: number[];
  triggered: number;
  total_rules: number;
  top_rule_id: string;
};

export type CoachFinding = {
  rule_id: string;
  group: string;
  severity: string;
  occurrences: number;
  total: number;
  pct: string;
  stat: string;
  examples: FindingExample[];
};

export type FindingExample = {
  text: string;
  detail: string;
};

export type FlowSummary = {
  overall_score: number;
  label_id: string;
  avg_followup: string;
  avg_block: string;
  deep_days: number;
  fragmented_days: number;
  total_days: number;
  days: FlowDayMetric[];
};

export type FlowDayMetric = {
  day: string;
  score: number;
  label_id: string;
  longest_block_min: number;
  active_min: number;
  sessions: number;
};

export type PaceSummary = {
  current_streak: number;
  longest_streak: number;
  late_night_pct: number;
  weekend_pct: number;
  risk_id: string;
  alert_ids: string[];
};

export type OutputSummary = {
  total_loc: string;
  by_language: CountMetric[];
  by_day: CountMetric[];
  trend: CountMetric[];
  by_project: CountMetric[];
  by_model: CountMetric[];
  uncovered_tools: string;
};

export type CoachTimelineDay = {
  day: string;
  max_concurrent: number;
  window_start_min: number;
  window_end_min: number;
  /** Formatted spend across the day's sessions. */
  total_cost: string;
  rows: TimelineSessionRow[];
};

export type TimelineSessionRow = {
  session_key: string;
  project: string;
  tool: string;
  tool_label: string;
  turns: number;
  cost: string;
  blocks: TimelineBlock[];
};

export type TimelineBlock = {
  start_min: number;
  end_min: number;
};

export type StackedDayMetric = {
  day: string;
  total_cost: string;
  segments: StackSegment[];
};

export type StackSegment = {
  tool: string;
  tool_label: string;
  cost: string;
  amount: number;
};

export type ShareMetric = {
  key: string;
  label: string;
  cost: string;
  calls: number;
  share: number;
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
  /** Deterministic task categories classified per call. */
  by_activity: ActivityMetric[];
  /** `tool · model` pairs billed at the fallback pricing rate. */
  fallback_priced_models: string[];
};

export type LimitMetric = {
  tool: string;
  scope: string;
  window: string;
  used: number;
  left: string;
  reset: string;
  plan: string;
  used_credits: number | null;
  remaining_credits: number | null;
  total_credits: number | null;
  additional_usage: boolean | null;
  stale: boolean;
  as_of: string;
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
  provider: string;
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
  plan_value: PlanValueMetric | null;
};

/** API-equivalent month spend vs the tool's subscription price. */
export type PlanValueMetric = {
  price: string;
  month_cost: string;
  multiple: string;
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
  interaction_mode: string;
  token_quality: string;
  timestamp_quality: string;
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

/** Mirrors `src/doctor.rs` `Verdict` (serde snake_case). */
export type DoctorVerdict = 'ok' | 'nothing_found' | 'errors' | 'discovery_failed';

export type DoctorEnvOverride = {
  name: string;
  value: string | null;
};

export type DoctorRootStatus = {
  label: string;
  path: string;
  exists: boolean;
};

/** Mirrors `src/doctor.rs` `ToolReport`. */
export type DoctorToolReport = {
  id: string;
  name: string;
  env: DoctorEnvOverride[];
  roots: DoctorRootStatus[];
  session_sources: number;
  limit_sources: number;
  sampled_sources: number;
  sampled_calls: number;
  sampled_limit_snapshots: number;
  parse_errors: number;
  verdict: DoctorVerdict;
  detail: string | null;
};

export type DoctorReport = {
  tools: DoctorToolReport[];
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
  plan_prices: PlanPriceRow[];
};

export type PlanPriceRow = {
  id: string;
  label: string;
  price: number | null;
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

export type ShortcutHint = {
  keys: string;
  label: string;
  action: string;
};

export type CopyHintGroup = {
  title: string;
  items: ShortcutHint[];
};

export type CoachRuleCopy = {
  name: string;
  description: string;
  when_triggered: string;
  how_to_improve: string;
};

export type CoachCopy = {
  report: {
    overall: string;
    grade_labels: Record<string, string>;
  };
  hero: {
    rules_clean: string;
    findings: string;
    high_severity: string;
    streak: string;
    total_loc: string;
  };
  tips: {
    title: string;
  };
  tabs: {
    label: string;
    report: string;
    findings: string;
    output: string;
    activity: string;
  };
  groups: Record<string, string>;
  score: Record<string, string>;
  findings: {
    title: string;
    priority: string;
    all: string;
    high: string;
    medium: string;
    affected_practices: string;
    total_occurrences: string;
    empty: string;
    occurrences: string;
    improve: string;
    examples: string;
    more_examples: string;
    filter_label: string;
    selected: string;
    occurrences_label: string;
    sample_size: string;
    trigger_rate: string;
    evidence_count: string;
    why_it_matters: string;
    no_examples: string;
    severity: Record<string, string>;
  };
  setup: {
    heading: string;
    subtitle: string;
    savings: string;
    empty: string;
    unused_mcp_title: string;
    unused_mcp_detail: string;
    claude_md_title: string;
    claude_md_detail: string;
    rereads_title: string;
    rereads_detail: string;
    junk_title: string;
    junk_detail: string;
  };
  flow: {
    title: string;
    labels: Record<string, string>;
    avg_followup: string;
    avg_block: string;
    deep_days: string;
    fragmented_days: string;
    trend: string;
    chart_aria: string;
    day_detail: string;
    empty: string;
  };
  pace: {
    title: string;
    streak: string;
    longest_streak: string;
    late_night: string;
    weekend: string;
    risk: string;
    days_suffix: string;
    risks: Record<string, string>;
    alerts: Record<string, string>;
  };
  timeline: Record<string, string>;
  output: Record<string, string>;
  rules: Record<string, CoachRuleCopy>;
};

export type CopyDeck = {
  brand: Record<string, string>;
  nav: Record<string, string>;
  coach: CoachCopy;
  periods: Record<string, string>;
  sorts: Record<string, string>;
  tools: Record<string, string>;
  metrics: Record<string, string>;
  filters: Record<string, string>;
  panels: Record<string, string>;
  categories: Record<string, string>;
  doctor: Record<string, string>;
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
  keymap: {
    actions: Record<string, string>;
    help: CopyHintGroup[];
    footers: Record<string, ShortcutHint[]>;
  };
  status: Record<string, string>;
};

export type DesktopSnapshot = {
  copy: CopyDeck;
  version: string;
  data_generation: number;
  source: 'live' | 'sample';
  status: string | null;
  status_tone: 'info' | 'busy' | 'success' | 'warning' | 'error';
  period: PeriodId;
  periods: OptionItem<PeriodId>[];
  tool: ToolId;
  tools: OptionItem<ToolId>[];
  sort: SortId;
  sorts: OptionItem<SortId>[];
  project: ProjectState;
  dashboard: DashboardData;
  usage: LimitsData;
  projects: ProjectOption[];
  report_projects: ProjectOption[];
  sessions: SessionOption[];
  config_rows: ConfigRow[];
  currencies: string[];
  currency: string;
  desktop_settings: DesktopSettingsState;
  desktop_updates: DesktopUpdateState;
  report_dir: string;
  report_formats: OptionItem<ReportFormatId>[];
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

export type ToolPageData = {
  dashboard: DashboardData;
  usage: LimitsData;
};

/** Mirrors `ProjectIndexRow` in `src/data/mod.rs`. */
export type ProjectIndexRow = {
  identity: string;
  name: string;
  cost: string;
  avg_per_session: string;
  sessions: number;
  calls: number;
  last_active: string;
  tool_mix: string;
  value: number;
};

/** Mirrors `ProjectInventoryRow` in `src/ingest/pipeline.rs`. */
export type ProjectSourceRow = {
  identity: string;
  project: string;
  tool: string;
  raw_project: string;
  calls: number;
  sessions: number;
  cost: string;
};

/** Mirrors `ProjectToolSplit` in `src/data/mod.rs`. */
export type ProjectToolSplit = {
  key: string;
  label: string;
  cost: string;
  avg_per_session: string;
  calls: number;
  sessions: number;
  cost_value: number;
  avg_value: number;
};

/** Mirrors `ProjectPageData` in `desktop/src-tauri/src/snapshot.rs`. */
export type ProjectPageData = {
  identity: string;
  name: string;
  dashboard: DashboardData;
  sessions: SessionOption[];
  sources: ProjectSourceRow[];
  tool_split: ProjectToolSplit[];
  output: OutputSummary;
  activity: CoachProjectActivity | null;
};
