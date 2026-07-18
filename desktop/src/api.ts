import { type Channel, invoke } from '@tauri-apps/api/core';
import type {
  AnalyticsData,
  CoachData,
  CoachTimelineDay,
  DesktopSnapshot,
  DesktopUpdateDownloadEvent,
  DesktopUpdateMetadata,
  DoctorReport,
  ModelCatalogEntry,
  ModelPageData,
  PeriodId,
  ProjectIndexRow,
  ProjectOption,
  ProjectPageData,
  ReportFormatId,
  ReportResponse,
  ScrollbackResults,
  SessionDetailView,
  SortId,
  ToolPageData,
  TraySnapshot,
  ToolId
} from './types';

export const api = {
  snapshot: () => invoke<DesktopSnapshot>('get_snapshot'),
  traySnapshot: () => invoke<TraySnapshot>('get_tray_snapshot'),
  openMainWindow: () => invoke<void>('open_main_window'),
  hideTrayPopover: () => invoke<void>('hide_tray_popover'),
  setPeriod: (period: PeriodId) => invoke<DesktopSnapshot>('set_period', { period }),
  setTool: (tool: ToolId) => invoke<DesktopSnapshot>('set_tool', { tool }),
  setSort: (sort: SortId) => invoke<DesktopSnapshot>('set_sort', { sort }),
  setProject: (identity: string | null) => invoke<DesktopSnapshot>('set_project', { identity }),
  setCurrency: (code: string) => invoke<DesktopSnapshot>('set_currency', { code }),
  setPlanPrice: (tool: string, price: number | null) =>
    invoke<DesktopSnapshot>('set_plan_price', { tool, price }),
  getModelCatalog: (period: PeriodId) =>
    invoke<ModelCatalogEntry[]>('get_model_catalog', { period }),
  getToolPage: (tool: ToolId) => invoke<ToolPageData>('get_tool_page', { tool }),
  getProjectIndex: () => invoke<ProjectIndexRow[]>('get_project_index'),
  getProjectPage: (identity: string) => invoke<ProjectPageData>('get_project_page', { identity }),
  getModelPage: (canonicalId: string) => invoke<ModelPageData>('get_model_page', { canonicalId }),
  getAnalytics: (period: PeriodId) => invoke<AnalyticsData>('get_analytics', { period }),
  getCoach: (period: PeriodId) => invoke<CoachData>('get_coach', { period }),
  getCoachTimeline: (day: string) =>
    invoke<CoachTimelineDay | null>('get_coach_timeline', { day }),
  getSessionDetail: (key: string) =>
    invoke<SessionDetailView | null>('get_session_detail', { key }),
  searchTranscripts: (args: {
    query: string;
    project?: string | null;
    tool?: string | null;
    limit?: number;
  }) => invoke<ScrollbackResults>('search_transcripts', args),
  setOpenAtLogin: (enabled: boolean) =>
    invoke<DesktopSnapshot>('set_open_at_login', { enabled }),
  setMcpHttpEnabled: (enabled: boolean) =>
    invoke<DesktopSnapshot>('set_mcp_http_enabled', { enabled }),
  setMcpHttpPort: (port: number) => invoke<DesktopSnapshot>('set_mcp_http_port', { port }),
  revealMcpToken: () => invoke<string>('reveal_mcp_token'),
  setShowDockOrTaskbarIcon: (enabled: boolean) =>
    invoke<DesktopSnapshot>('set_show_dock_or_taskbar_icon', { enabled }),
  refreshArchive: () => invoke<DesktopSnapshot>('refresh_archive'),
  clearData: () => invoke<DesktopSnapshot>('clear_data'),
  refreshCurrencyRates: () => invoke<DesktopSnapshot>('refresh_currency_rates'),
  refreshPricingSnapshot: () => invoke<DesktopSnapshot>('refresh_pricing_snapshot'),
  syncClaudeLimits: () => invoke<DesktopSnapshot>('sync_claude_limits'),
  installClaudeStatusline: () => invoke<DesktopSnapshot>('install_claude_statusline'),
  installClaudeStatuslineManual: () =>
    invoke<DesktopSnapshot>('install_claude_statusline_manual'),
  uninstallClaudeStatusline: () => invoke<DesktopSnapshot>('uninstall_claude_statusline'),
  syncCopilotLimits: () => invoke<DesktopSnapshot>('sync_copilot_limits'),
  syncClaudeSubscriptionLimits: () =>
    invoke<DesktopSnapshot>('sync_claude_subscription_limits'),
  syncCodexSubscriptionLimits: () =>
    invoke<DesktopSnapshot>('sync_codex_subscription_limits'),
  setClaudeSessionCookie: (value: string) =>
    invoke<DesktopSnapshot>('set_claude_session_cookie', { value }),
  clearClaudeSessionCookie: () =>
    invoke<DesktopSnapshot>('clear_claude_session_cookie'),
  setCodexSessionCookie: (value: string) =>
    invoke<DesktopSnapshot>('set_codex_session_cookie', { value }),
  clearCodexSessionCookie: () =>
    invoke<DesktopSnapshot>('clear_codex_session_cookie'),
  checkDesktopUpdate: () => invoke<DesktopUpdateMetadata | null>('check_desktop_update'),
  installDesktopUpdate: (onEvent: Channel<DesktopUpdateDownloadEvent>) =>
    invoke<void>('install_desktop_update', { onEvent }),
  getDoctor: () => invoke<DoctorReport>('get_doctor'),
  setReportDir: (path: string) => invoke<DesktopSnapshot>('set_report_dir', { path }),
  reportProjects: (period: PeriodId) => invoke<ProjectOption[]>('report_projects', { period }),
  generateReport: (
    format: ReportFormatId,
    period: PeriodId,
    projectIdentity: string | null,
    redacted: boolean
  ) =>
    invoke<ReportResponse>('generate_report', {
      format,
      period,
      projectIdentity,
      redacted
    }),
  toggleDataSource: () => invoke<DesktopSnapshot>('toggle_data_source')
};
