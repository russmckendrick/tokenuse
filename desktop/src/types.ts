export type PageId = 'overview' | 'deep-dive' | 'usage' | 'config' | 'session';
export type PeriodId = 'today' | 'week' | 'thirty-days' | 'month' | 'all-time';
export type ToolId = 'all' | 'claude-code' | 'cursor' | 'codex' | 'copilot';
export type SortId = 'spend' | 'date' | 'tokens';
export type ExportFormatId = 'json' | 'csv' | 'svg' | 'png';

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
  name: string;
  value: string;
  action: string;
};

export type ProjectState = {
  identity: string | null;
  label: string;
};

export type ShortcutHint = {
  keys: string;
  label: string;
  action: string;
};

export type ShortcutInput = {
  key: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
};

export type DesktopSnapshot = {
  version: string;
  source: 'live' | 'sample';
  status: string | null;
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
  projects: ProjectOption[];
  sessions: SessionOption[];
  session: SessionDetailView | null;
  config_rows: ConfigRow[];
  currencies: string[];
  currency: string;
  export_dir: string;
  export_formats: OptionItem<ExportFormatId>[];
  shortcut_footer: ShortcutHint[];
};

export type ExportResponse = {
  path: string;
  snapshot: DesktopSnapshot;
};

export type ShortcutResponse = {
  handled: boolean;
  effect: 'open_project_picker' | 'open_session_picker' | 'open_export_picker' | 'close_modal' | 'close_call_detail' | null;
  snapshot: DesktopSnapshot;
};
