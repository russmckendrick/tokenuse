export type PageId = 'overview' | 'deep-dive' | 'usage' | 'insights' | 'config' | 'session';
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
    advice_latest: string;
    advice_run: string;
    advice_confidence: string;
    advice_next_step: string;
    advice_evidence: string;
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
