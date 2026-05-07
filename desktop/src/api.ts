import { type Channel, invoke } from '@tauri-apps/api/core';
import type {
  DesktopSnapshot,
  DesktopUpdateDownloadEvent,
  DesktopUpdateMetadata,
  PageId,
  PeriodId,
  ProjectOption,
  ReportFormatId,
  ReportResponse,
  ShortcutInput,
  ShortcutResponse,
  SortId,
  TraySnapshot,
  ToolId
} from './types';

export const api = {
  snapshot: () => invoke<DesktopSnapshot>('get_snapshot'),
  traySnapshot: () => invoke<TraySnapshot>('get_tray_snapshot'),
  openMainWindow: () => invoke<void>('open_main_window'),
  hideTrayPopover: () => invoke<void>('hide_tray_popover'),
  setPage: (page: PageId) => invoke<DesktopSnapshot>('set_page', { page }),
  setPeriod: (period: PeriodId) => invoke<DesktopSnapshot>('set_period', { period }),
  setTool: (tool: ToolId) => invoke<DesktopSnapshot>('set_tool', { tool }),
  setSort: (sort: SortId) => invoke<DesktopSnapshot>('set_sort', { sort }),
  setProject: (identity: string | null) => invoke<DesktopSnapshot>('set_project', { identity }),
  openSession: (key: string) => invoke<DesktopSnapshot>('open_session', { key }),
  closeSession: () => invoke<DesktopSnapshot>('close_session'),
  setCurrency: (code: string) => invoke<DesktopSnapshot>('set_currency', { code }),
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
  handleShortcut: (context: string, input: ShortcutInput) =>
    invoke<ShortcutResponse>('handle_shortcut', { context, input })
};
