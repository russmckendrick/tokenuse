import { type Channel, invoke } from '@tauri-apps/api/core';
import type {
  AnalyticsData,
  DesktopSnapshot,
  DesktopUpdateDownloadEvent,
  DesktopUpdateMetadata,
  ModelCatalogEntry,
  PeriodId,
  ProjectOption,
  ReportFormatId,
  ReportResponse,
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
  getModelCatalog: (period: PeriodId) =>
    invoke<ModelCatalogEntry[]>('get_model_catalog', { period }),
  getToolPage: (tool: ToolId) => invoke<ToolPageData>('get_tool_page', { tool }),
  getAnalytics: (period: PeriodId) => invoke<AnalyticsData>('get_analytics', { period }),
  getSessionDetail: (key: string) =>
    invoke<SessionDetailView | null>('get_session_detail', { key }),
  setOpenAtLogin: (enabled: boolean) =>
    invoke<DesktopSnapshot>('set_open_at_login', { enabled }),
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
