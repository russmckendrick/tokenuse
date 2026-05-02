import { invoke } from '@tauri-apps/api/core';
import type {
  DesktopSnapshot,
  ExportFormatId,
  ExportResponse,
  PageId,
  PeriodId,
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
  refreshCurrencyRates: () => invoke<DesktopSnapshot>('refresh_currency_rates'),
  refreshPricingSnapshot: () => invoke<DesktopSnapshot>('refresh_pricing_snapshot'),
  setExportDir: (path: string) => invoke<DesktopSnapshot>('set_export_dir', { path }),
  exportCurrent: (format: ExportFormatId) =>
    invoke<ExportResponse>('export_current', { format }),
  handleShortcut: (context: string, input: ShortcutInput) =>
    invoke<ShortcutResponse>('handle_shortcut', { context, input })
};
