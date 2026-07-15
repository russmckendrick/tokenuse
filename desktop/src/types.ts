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
  practice_groups: PracticeGroupScore[];
  findings: CoachFinding[];
  flow: FlowSummary;
  pace: PaceSummary;
  output: OutputSummary;
  timeline_days: string[];
};

export type PracticeGroupScore = {
  id: string;
  score: number;
  wow: string;
  mom: string;
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
  by_project: CountMetric[];
  by_model: CountMetric[];
  uncovered_tools: string;
};

export type CoachTimelineDay = {
  day: string;
  max_concurrent: number;
  window_start_min: number;
  window_end_min: number;
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
  groups: Record<string, string>;
  score: Record<string, string>;
  findings: {
    title: string;
    empty: string;
    occurrences: string;
    improve: string;
    examples: string;
    severity: Record<string, string>;
  };
  flow: {
    title: string;
    labels: Record<string, string>;
    avg_followup: string;
    avg_block: string;
    deep_days: string;
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

