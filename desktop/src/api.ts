import { invoke } from '@tauri-apps/api/core';
import type {
  DesktopSnapshot,
  ExportFormatId,
  ExportResponse,
  PageId,
  PeriodId,
  ToolId
} from './types';

export const api = {
  snapshot: () => invoke<DesktopSnapshot>('get_snapshot'),
  setPage: (page: PageId) => invoke<DesktopSnapshot>('set_page', { page }),
  setPeriod: (period: PeriodId) => invoke<DesktopSnapshot>('set_period', { period }),
  setTool: (tool: ToolId) => invoke<DesktopSnapshot>('set_tool', { tool }),
  setProject: (identity: string | null) => invoke<DesktopSnapshot>('set_project', { identity }),
  openSession: (key: string) => invoke<DesktopSnapshot>('open_session', { key }),
  closeSession: () => invoke<DesktopSnapshot>('close_session'),
  setCurrency: (code: string) => invoke<DesktopSnapshot>('set_currency', { code }),
  refreshArchive: () => invoke<DesktopSnapshot>('refresh_archive'),
  refreshCurrencyRates: () => invoke<DesktopSnapshot>('refresh_currency_rates'),
  refreshPricingSnapshot: () => invoke<DesktopSnapshot>('refresh_pricing_snapshot'),
  setExportDir: (path: string) => invoke<DesktopSnapshot>('set_export_dir', { path }),
  exportCurrent: (format: ExportFormatId) =>
    invoke<ExportResponse>('export_current', { format })
};
