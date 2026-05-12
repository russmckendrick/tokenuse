<script lang="ts">
  import { onMount } from 'svelte';
  import { Channel } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { confirm, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { Download, FolderOpen, RefreshCw, Search, X } from 'lucide-svelte';
  import { api } from './api';
  import { count } from './format';
  import { fadeIn, pill, reveal } from './motion';
  import TrayPopover from './TrayPopover.svelte';
  import ConfigView from './views/ConfigView.svelte';
  import DeepDiveView from './views/DeepDiveView.svelte';
  import InsightsView from './views/InsightsView.svelte';
  import OverviewView from './views/OverviewView.svelte';
  import SessionView from './views/SessionView.svelte';
  import UsageView from './views/UsageView.svelte';
  import type {
    AdviceDataScopeId,
    AdviceItemStatusId,
    ConfigRow,
    DesktopSnapshot,
    DesktopUpdateDownloadEvent,
    DesktopUpdateMetadata,
    PageId,
    PeriodId,
    ProjectOption,
    ReportFormatId,
    ShortcutInput,
    SortId,
    SessionDetail,
    SessionOption,
    ToolId
  } from './types';

  type ModalKind =
    | 'project'
    | 'session'
    | 'currency'
    | 'advice_tool'
    | 'report'
    | 'subscription_cookie'
    | null;
  type SubscriptionProvider = 'claude' | 'codex';
  type DesktopUpdateUiState = {
    checking: boolean;
    installing: boolean;
    checked: boolean;
    available: DesktopUpdateMetadata | null;
    message: string | null;
    downloaded: number;
    total: number | null;
  };

  function currentWindowLabel() {
    try {
      return getCurrentWindow().label;
    } catch {
      return 'main';
    }
  }

  const isTrayPopover = currentWindowLabel() === 'tray-popover';

  let snapshot: DesktopSnapshot | null = null;
  let busy = false;
  let error: string | null = null;
  let modal: ModalKind = null;
  let cookieProvider: SubscriptionProvider | null = null;
  let cookieValue = '';
  let codexShard0 = '';
  let codexShard1 = '';
  let codexExtraCookies = '';
  let cookieBusy = false;
  let cookieError: string | null = null;
  let statusExpanded = false;
  let callDetail: SessionDetail | null = null;
  let query = '';
  let reportFormat: ReportFormatId = 'html';
  let reportPeriod: PeriodId = 'week';
  let reportProjectIdentity = '';
  let reportProjects: ProjectOption[] = [];
  let reportRedacted = false;
  let insightsGenerateRequest = 0;
  let clearingData = false;
  let pollTimer: number | undefined;
  let desktopUpdate: DesktopUpdateUiState = resetDesktopUpdate();

  function resetDesktopUpdate(): DesktopUpdateUiState {
    return {
      checking: false,
      installing: false,
      checked: false,
      available: null,
      message: null,
      downloaded: 0,
      total: null
    };
  }

  onMount(() => {
    if (isTrayPopover) return;

    void load();
    pollTimer = window.setInterval(() => void loadSilent(), 3000);
    window.addEventListener('keydown', handleKey);

    return () => {
      if (pollTimer !== undefined) {
        window.clearInterval(pollTimer);
      }
      window.removeEventListener('keydown', handleKey);
    };
  });

  async function load() {
    await commit(() => api.snapshot());
  }

  async function loadSilent() {
    try {
      snapshot = await api.snapshot();
    } catch {
      // Keep the last good render during transient backend errors.
    }
  }

  async function commit(action: () => Promise<DesktopSnapshot>) {
    busy = true;
    error = null;
    try {
      snapshot = await action();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function openModal(kind: Exclude<ModalKind, null>) {
    modal = kind;
    query = '';
    if (kind === 'report') {
      reportFormat = snapshot?.report_formats[0]?.value ?? 'html';
      reportPeriod = snapshot?.period ?? 'week';
      reportProjects = snapshot?.report_projects ?? [];
      const currentProject = snapshot?.project.identity ?? '';
      reportProjectIdentity = reportProjects.some((project) => project.identity === currentProject)
        ? currentProject
        : '';
      reportRedacted = false;
    }
  }

  function openCookieModal(provider: SubscriptionProvider) {
    cookieProvider = provider;
    cookieValue = '';
    codexShard0 = '';
    codexShard1 = '';
    codexExtraCookies = '';
    cookieError = null;
    cookieBusy = false;
    openModal('subscription_cookie');
  }

  function composedCookieValue(): string {
    if (cookieProvider === 'codex') {
      const s0 = codexShard0.trim();
      const s1 = codexShard1.trim();
      if (!s0 || !s1) return '';
      const parts = [
        `__Secure-next-auth.session-token.0=${s0}`,
        `__Secure-next-auth.session-token.1=${s1}`
      ];
      const extra = codexExtraCookies.trim().replace(/^Cookie:\s*/i, '');
      if (extra) parts.push(extra);
      return parts.join('; ');
    }
    return cookieValue.trim();
  }

  function cookieFormReady(): boolean {
    if (cookieProvider === 'codex') {
      return codexShard0.trim().length > 0 && codexShard1.trim().length > 0;
    }
    return cookieValue.trim().length > 0;
  }

  function cookieIsSet(provider: SubscriptionProvider | null): boolean {
    if (!snapshot || !provider) return false;
    return provider === 'claude'
      ? snapshot.subscription_cookies.claude_set
      : snapshot.subscription_cookies.codex_set;
  }

  function cookieProviderLabel(provider: SubscriptionProvider | null): string {
    return provider === 'codex' ? 'ChatGPT (Codex)' : 'Claude.ai';
  }

  async function saveAndSyncCookie() {
    if (!cookieProvider) return;
    const composed = composedCookieValue();
    if (!composed) {
      cookieError =
        cookieProvider === 'codex'
          ? 'Paste both __Secure-next-auth.session-token.0 and .1 shards.'
          : 'Paste the cookie value first.';
      return;
    }
    cookieBusy = true;
    cookieError = null;
    try {
      snapshot =
        cookieProvider === 'claude'
          ? await api.setClaudeSessionCookie(composed)
          : await api.setCodexSessionCookie(composed);
      snapshot =
        cookieProvider === 'claude'
          ? await api.syncClaudeSubscriptionLimits()
          : await api.syncCodexSubscriptionLimits();
      cookieValue = '';
      codexShard0 = '';
      codexShard1 = '';
      codexExtraCookies = '';
      closeModal();
    } catch (err) {
      cookieError = err instanceof Error ? err.message : String(err);
    } finally {
      cookieBusy = false;
    }
  }

  async function syncWithStoredCookie() {
    if (!cookieProvider) return;
    cookieBusy = true;
    cookieError = null;
    try {
      snapshot =
        cookieProvider === 'claude'
          ? await api.syncClaudeSubscriptionLimits()
          : await api.syncCodexSubscriptionLimits();
      closeModal();
    } catch (err) {
      cookieError = err instanceof Error ? err.message : String(err);
    } finally {
      cookieBusy = false;
    }
  }

  async function clearStoredCookie() {
    if (!cookieProvider) return;
    cookieBusy = true;
    cookieError = null;
    try {
      snapshot =
        cookieProvider === 'claude'
          ? await api.clearClaudeSessionCookie()
          : await api.clearCodexSessionCookie();
    } catch (err) {
      cookieError = err instanceof Error ? err.message : String(err);
    } finally {
      cookieBusy = false;
    }
  }

  function closeModal() {
    modal = null;
    query = '';
  }

  function openCallDetail(call: SessionDetail) {
    modal = null;
    query = '';
    callDetail = call;
  }

  function closeCallDetail() {
    callDetail = null;
  }

  function handleCallRowKey(event: KeyboardEvent, call: SessionDetail) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openCallDetail(call);
    }
  }

  function handleKey(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if ((target?.tagName === 'INPUT' || target?.tagName === 'SELECT') && event.key !== 'Escape') {
      return;
    }
    if (!snapshot) return;

    void commitShortcut(event);
  }

  async function commitShortcut(event: KeyboardEvent) {
    busy = true;
    error = null;
    try {
      const response = await api.handleShortcut(shortcutContext(event), shortcutInput(event));
      if (!response.handled) return;
      event.preventDefault();
      snapshot = response.snapshot;
      applyShortcutEffect(response.effect);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function shortcutContext(event: KeyboardEvent) {
    if (callDetail) return 'desktop_call_detail';
    if (modal) return 'desktop_modal';
    if (snapshot?.page === 'session' && event.key === 'Escape') return 'desktop_session_page';
    if (snapshot?.page === 'usage') return 'desktop_usage_page';
    if (snapshot?.page === 'insights') return 'desktop_insights_page';
    if (snapshot?.page === 'config') return 'desktop_config_page';
    return 'desktop';
  }

  function shortcutInput(event: KeyboardEvent): ShortcutInput {
    return {
      key: event.key,
      ctrl: event.ctrlKey,
      alt: event.altKey,
      shift: event.shiftKey,
      meta: event.metaKey
    };
  }

  function applyShortcutEffect(effect: string | null) {
    switch (effect) {
      case 'open_project_picker':
        openModal('project');
        break;
      case 'open_session_picker':
        openModal('session');
        break;
      case 'open_export_picker':
        openModal('report');
        break;
      case 'close_modal':
        closeModal();
        break;
      case 'close_call_detail':
        closeCallDetail();
        break;
      case 'generate_advice_selected':
        insightsGenerateRequest += 1;
        break;
    }
  }

  function setToolFromEvent(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as ToolId;
    void commit(() => api.setTool(value));
  }

  function setSortFromEvent(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as SortId;
    void commit(() => api.setSort(value));
  }

  function setOpenAtLoginFromEvent(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void commit(() => api.setOpenAtLogin(enabled));
  }

  function setShowDockOrTaskbarIconFromEvent(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void commit(() => api.setShowDockOrTaskbarIcon(enabled));
  }

  function filteredProjects(): ProjectOption[] {
    if (!snapshot) return [];
    const needle = query.trim().toLowerCase();
    return snapshot.projects.filter((project) => {
      return !needle || project.label.toLowerCase().includes(needle);
    });
  }

  function filteredSessions(): SessionOption[] {
    if (!snapshot) return [];
    const needle = query.trim().toLowerCase();
    return snapshot.sessions.filter((session) => {
      return (
        !needle ||
        session.project.toLowerCase().includes(needle) ||
        session.tool.toLowerCase().includes(needle) ||
        session.date.toLowerCase().includes(needle) ||
        session.key.toLowerCase().includes(needle)
      );
    });
  }

  function filteredCurrencies(): string[] {
    if (!snapshot) return [];
    const needle = query.trim().toLowerCase();
    return snapshot.currencies.filter((currency) => !needle || currency.toLowerCase().includes(needle));
  }

  function filteredAdviceTools() {
    if (!snapshot) return [];
    const needle = query.trim().toLowerCase();
    return snapshot.advice_tool_options.filter((tool) => !needle || tool.label.toLowerCase().includes(needle));
  }

  async function chooseReportDir() {
    if (!snapshot) return;
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: snapshot.report_dir
    });
    if (typeof selected === 'string') {
      await commit(() => api.setReportDir(selected));
    }
  }

  async function runReport() {
    busy = true;
    error = null;
    try {
      const result = await api.generateReport(reportFormat, reportPeriod, reportProjectIdentity || null, reportRedacted);
      snapshot = result.snapshot;
      closeModal();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function selectReportPeriod(period: PeriodId) {
    reportPeriod = period;
    try {
      reportProjects = await api.reportProjects(period);
      if (!reportProjects.some((project) => project.identity === reportProjectIdentity)) {
        reportProjectIdentity = '';
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function activePage() {
    return snapshot?.page ?? 'overview';
  }

  function activeSortLabel() {
    if (!snapshot) return '';
    return snapshot.sorts.find((sort) => sort.value === snapshot?.sort)?.label ?? '';
  }

  function isUsagePage() {
    return activePage() === 'usage';
  }

  function isConfigPage() {
    return activePage() === 'config';
  }

  function isInsightsPage() {
    return activePage() === 'insights';
  }

  function isPeriodDisabled(period: PeriodId) {
    return isConfigPage() || isInsightsPage() || (isUsagePage() && period !== 'today');
  }

  function isToolDisabled() {
    return isConfigPage() || isUsagePage() || isInsightsPage();
  }

  function isSortDisabled() {
    return isConfigPage() || isUsagePage() || isInsightsPage();
  }

  function isProjectDisabled() {
    return isConfigPage() || isUsagePage() || isInsightsPage();
  }

  function tabsFor(state: DesktopSnapshot): Array<{ value: PageId; label: string }> {
    return [
      { value: 'overview', label: state.copy.nav.overview },
      { value: 'deep-dive', label: state.copy.nav.deep_dive },
      { value: 'usage', label: state.copy.nav.usage },
      { value: 'insights', label: state.copy.nav.insights },
      { value: 'config', label: state.copy.nav.config }
    ];
  }

  function modalTitle(kind: Exclude<ModalKind, null>) {
    if (!snapshot) return kind;
    if (kind === 'report') return snapshot.copy.reports.modal_title;
    if (kind === 'advice_tool') return snapshot.copy.config.rows.advice_tool.name;
    if (kind === 'subscription_cookie') {
      return cookieProvider === 'codex'
        ? snapshot.copy.modals.sync_codex_subscription_limits_title
        : snapshot.copy.modals.sync_claude_subscription_limits_title;
    }
    return snapshot.copy.modals[kind] ?? kind;
  }

  function usageTone(tool: string, index: number) {
    const normalized = tool.toLowerCase();
    if (normalized.includes('codex')) return 'orange';
    if (normalized.includes('claude')) return 'magenta';
    if (normalized.includes('cursor')) return 'blue';
    if (normalized.includes('copilot')) return 'green';
    if (normalized.includes('gemini')) return 'cyan';
    return ['cyan', 'yellow', 'magenta', 'green'][index % 4];
  }

  function configAction(row: ConfigRow) {
    void runConfigAction(row);
  }

  async function runConfigAction(row: ConfigRow) {
    if (!snapshot) return;
    switch (row.id) {
      case 'currency_override':
        openModal('currency');
        break;
      case 'rates_json':
        if (await confirmDownload(snapshot.copy.modals.download_rates_title, snapshot.copy.modals.download_latest_rates_message)) {
          await commit(() => api.refreshCurrencyRates());
        }
        break;
      case 'litellm_prices':
        if (await confirmDownload(snapshot.copy.modals.download_prices_title, snapshot.copy.modals.download_latest_prices_message)) {
          await commit(() => api.refreshPricingSnapshot());
        }
        break;
      case 'claude_statusline':
        await runClaudeStatuslineAction(row);
        break;
      case 'claude_limits':
        await commit(() => api.syncClaudeLimits());
        break;
      case 'copilot_limits':
        if (
          await confirmDownload(
            snapshot.copy.modals.sync_copilot_limits_title,
            snapshot.copy.modals.sync_copilot_limits_message,
            snapshot.copy.actions.sync
          )
        ) {
          await commit(() => api.syncCopilotLimits());
        }
        break;
      case 'claude_subscription_limits':
        openCookieModal('claude');
        break;
      case 'codex_subscription_limits':
        openCookieModal('codex');
        break;
      case 'advice_tool':
        openModal('advice_tool');
        break;
      case 'advice_prompts':
        await commit(() => api.prepareAdvicePrompts());
        break;
      case 'clear_data':
        if (await confirmClearData()) {
          await runClearData();
        }
        break;
    }
  }

  async function runClaudeStatuslineAction(row: ConfigRow) {
    if (!snapshot) return;
    const installedPrefix = snapshot.copy.config.values.statusline_installed_passthrough.split(' · ')[0];
    const isInstalled = row.value.startsWith(installedPrefix);
    const c = snapshot.copy;
    if (isInstalled) {
      try {
        const ok = await confirm(c.modals.uninstall_claude_statusline_message, {
          title: c.modals.uninstall_claude_statusline_title,
          kind: 'warning',
          okLabel: c.actions.uninstall,
          cancelLabel: c.actions.cancel
        });
        if (ok) await commit(() => api.uninstallClaudeStatusline());
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      }
      return;
    }
    try {
      const ok = await confirm(c.modals.install_claude_statusline_message, {
        title: c.modals.install_claude_statusline_title,
        kind: 'info',
        okLabel: c.actions.install,
        cancelLabel: c.actions.cancel
      });
      if (ok) {
        await commit(() => api.installClaudeStatusline());
        return;
      }
      const manual = await confirm(c.modals.install_claude_statusline_manual_message, {
        title: c.modals.install_claude_statusline_manual_title,
        kind: 'info',
        okLabel: c.actions.install_manual,
        cancelLabel: c.actions.cancel
      });
      if (manual) await commit(() => api.installClaudeStatuslineManual());
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function confirmDownload(title: string, message: string, okLabel = snapshot?.copy.actions.download ?? '') {
    try {
      return await confirm(message, {
        title,
        kind: 'warning',
        okLabel,
        cancelLabel: snapshot?.copy.actions.cancel ?? ''
      });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      return false;
    }
  }

  async function confirmClearData() {
    if (!snapshot) return false;
    try {
      return await confirm(snapshot.copy.modals.clear_data_message, {
        title: snapshot.copy.modals.clear_data_question,
        kind: 'warning',
        okLabel: snapshot.copy.actions.clear_data,
        cancelLabel: snapshot.copy.actions.cancel
      });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      return false;
    }
  }

  async function runClearData() {
    busy = true;
    clearingData = true;
    error = null;
    try {
      snapshot = await api.clearData();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      clearingData = false;
      busy = false;
    }
  }

  function generateAdvice(dataScope: AdviceDataScopeId) {
    void commit(() => api.generateAdvice(dataScope));
  }

  function updateAdviceItemStatus(itemId: number, status: AdviceItemStatusId) {
    void commit(() => api.updateAdviceItemStatus(itemId, status));
  }

  async function checkDesktopUpdate() {
    if (!snapshot) return;
    busy = true;
    error = null;
    desktopUpdate = {
      ...resetDesktopUpdate(),
      checking: true,
      message: snapshot.copy.updates.checking
    };

    try {
      const update = await api.checkDesktopUpdate();
      desktopUpdate = {
        ...desktopUpdate,
        checking: false,
        checked: true,
        available: update,
        message: update
          ? copyTemplate(snapshot.copy.updates.available, { version: update.version })
          : snapshot.copy.updates.up_to_date
      };
    } catch (err) {
      desktopUpdate = {
        ...desktopUpdate,
        checking: false,
        checked: true,
        available: null,
        message: updateFailureMessage(err)
      };
    } finally {
      busy = false;
    }
  }

  async function installDesktopUpdate() {
    if (!snapshot || !desktopUpdate.available) return;
    busy = true;
    error = null;
    desktopUpdate = {
      ...desktopUpdate,
      installing: true,
      downloaded: 0,
      total: null,
      message: snapshot.copy.updates.installing
    };

    const onEvent = new Channel<DesktopUpdateDownloadEvent>();
    onEvent.onmessage = (event) => handleDesktopUpdateDownloadEvent(event);

    try {
      await api.installDesktopUpdate(onEvent);
      desktopUpdate = {
        ...desktopUpdate,
        installing: false,
        message: snapshot.copy.updates.installed_restarting
      };
    } catch (err) {
      desktopUpdate = {
        ...desktopUpdate,
        installing: false,
        message: updateFailureMessage(err)
      };
    } finally {
      busy = false;
    }
  }

  function handleDesktopUpdateDownloadEvent(event: DesktopUpdateDownloadEvent) {
    if (!snapshot) return;
    switch (event.event) {
      case 'started':
        desktopUpdate = {
          ...desktopUpdate,
          total: event.data.contentLength,
          message: snapshot.copy.updates.download_started
        };
        break;
      case 'progress': {
        const downloaded = desktopUpdate.downloaded + event.data.chunkLength;
        desktopUpdate = {
          ...desktopUpdate,
          downloaded,
          message: desktopUpdate.total === null
            ? copyTemplate(snapshot.copy.updates.download_progress_unknown, {
                downloaded: formatBytes(downloaded)
              })
            : copyTemplate(snapshot.copy.updates.download_progress, {
                downloaded: formatBytes(downloaded),
                total: formatBytes(desktopUpdate.total)
              })
        };
        break;
      }
      case 'finished':
        desktopUpdate = {
          ...desktopUpdate,
          message: snapshot.copy.updates.download_finished
        };
        break;
    }
  }

  function updateFailureMessage(err: unknown) {
    const detail = err instanceof Error ? err.message : String(err);
    return snapshot
      ? copyTemplate(snapshot.copy.updates.failed, { error: detail })
      : detail;
  }

  function copyTemplate(template: string, values: Record<string, string>) {
    return Object.entries(values).reduce(
      (out, [key, value]) => out.split(`{${key}}`).join(value),
      template
    );
  }

  function formatBytes(value: number) {
    const units = ['B', 'KB', 'MB', 'GB'];
    let amount = value;
    let index = 0;
    while (amount >= 1024 && index < units.length - 1) {
      amount /= 1024;
      index += 1;
    }
    return `${amount >= 10 || index === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[index]}`;
  }

  function statusMessage() {
    if (error) return error;
    if (clearingData) return snapshot?.copy.status.clearing_data_reimporting ?? null;
    return snapshot?.status ?? null;
  }

  function statusTone() {
    if (error) return 'error';
    if (clearingData) return 'busy';
    return snapshot?.status_tone ?? 'info';
  }
</script>

{#if isTrayPopover}
  <TrayPopover />
{:else if snapshot}
  <div class="app-shell" class:is-busy={busy}>
    <header class="topbar">
      <div class="brand">
        <svg class="brand-bars" viewBox="0 0 440 560" aria-hidden="true">
          <defs>
            <linearGradient id="brand-bar-gradient" x1="0" y1="0" x2="0" y2="560" gradientUnits="userSpaceOnUse">
              <stop offset="0%" stop-color="#ffc06a" />
              <stop offset="45%" stop-color="#ff9a4d" />
              <stop offset="100%" stop-color="#f26a3d" />
            </linearGradient>
          </defs>
          <rect x="0" y="280" width="80" height="280" rx="16" fill="url(#brand-bar-gradient)" />
          <rect x="120" y="160" width="80" height="400" rx="16" fill="url(#brand-bar-gradient)" />
          <rect x="240" y="0" width="80" height="560" rx="16" fill="url(#brand-bar-gradient)" />
          <rect x="360" y="120" width="80" height="440" rx="16" fill="url(#brand-bar-gradient)" />
        </svg>
        <span class="brand-title">{snapshot.copy.brand.name}</span>
      </div>

      <nav class="tabs" aria-label={snapshot.copy.desktop.sections_aria}>
        {#each tabsFor(snapshot) as tab}
          <button
            class:active={activePage() === tab.value}
            type="button"
            onclick={() => commit(() => api.setPage(tab.value))}
          >
            {tab.label}
          </button>
        {/each}
      </nav>

      <div class="actions">
        {#if statusMessage()}
          {#key statusTone() + (statusMessage() ?? '')}
            <button
              class="status-pill"
              class:error={statusTone() === 'error'}
              class:success={statusTone() === 'success'}
              class:warning={statusTone() === 'warning'}
              class:busy={statusTone() === 'busy'}
              class:is-expanded={statusExpanded}
              type="button"
              use:pill
              title={statusExpanded ? 'Click to collapse' : statusMessage() ?? ''}
              onclick={() => (statusExpanded = !statusExpanded)}
            >
              <i class="status-dot" aria-hidden="true"></i>
              <span>{statusMessage()}</span>
            </button>
          {/key}
        {/if}
        <button class="icon-button" type="button" title={snapshot.copy.actions.refresh_archive} onclick={() => commit(() => api.refreshArchive())}>
          <RefreshCw size={16} />
        </button>
        <button class="icon-button" type="button" title={snapshot.copy.actions.export_current_view} onclick={() => openModal('report')}>
          <Download size={16} />
        </button>
      </div>
    </header>

    <section class="filter-strip">
      <div class="segmented" aria-label={snapshot.copy.desktop.period_aria}>
        {#each snapshot.periods as period}
          <button
            type="button"
            class:active={snapshot.period === period.value}
            disabled={isPeriodDisabled(period.value)}
            onclick={() => commit(() => api.setPeriod(period.value))}
          >
            {period.label}
          </button>
        {/each}
      </div>

      <div class="filter-controls">
        <div class="segmented tool-strip" class:is-disabled={isToolDisabled()} aria-label={snapshot.copy.desktop.tool_aria}>
          <span>{snapshot.copy.filters.tool}</span>
          <select aria-label={snapshot.copy.desktop.tool_aria} disabled={isToolDisabled()} onchange={setToolFromEvent}>
            {#each snapshot.tools as tool}
              <option value={tool.value} selected={snapshot.tool === tool.value}>{tool.label}</option>
            {/each}
          </select>
        </div>

        <div class="segmented tool-strip sort-strip" class:is-disabled={isSortDisabled()} aria-label={snapshot.copy.desktop.sort_aria}>
          <span>{snapshot.copy.filters.sort}</span>
          <select aria-label={snapshot.copy.desktop.sort_aria} disabled={isSortDisabled()} onchange={setSortFromEvent}>
            {#each snapshot.sorts as sort}
              <option value={sort.value} selected={snapshot.sort === sort.value}>{sort.label}</option>
            {/each}
          </select>
        </div>

        <button class="segmented tool-strip project-strip" type="button" aria-label={snapshot.copy.desktop.project_aria} disabled={isProjectDisabled()} onclick={() => openModal('project')}>
          <span>{snapshot.copy.filters.project}</span>
          <strong>{snapshot.project.label}</strong>
        </button>
      </div>
    </section>

    <main>
      {#if activePage() === 'overview'}
        <OverviewView {snapshot} />
      {:else if activePage() === 'deep-dive'}
        <DeepDiveView {snapshot} openSessionPicker={() => openModal('session')} />
      {:else if activePage() === 'usage'}
        <UsageView {snapshot} {usageTone} />
      {:else if activePage() === 'insights'}
        <InsightsView
          {snapshot}
          {generateAdvice}
          {updateAdviceItemStatus}
          generateAdviceRequest={insightsGenerateRequest}
        />
      {:else if activePage() === 'config'}
        <ConfigView
          {snapshot}
          {configAction}
          chooseExportDir={chooseReportDir}
          refreshArchive={() => commit(() => api.refreshArchive())}
          {desktopUpdate}
          checkDesktopUpdate={() => void checkDesktopUpdate()}
          installDesktopUpdate={() => void installDesktopUpdate()}
          {setOpenAtLoginFromEvent}
          {setShowDockOrTaskbarIconFromEvent}
        />
      {:else if activePage() === 'session'}
        <SessionView
          {snapshot}
          session={snapshot.session}
          closeSession={() => commit(() => api.closeSession())}
          {openCallDetail}
          {handleCallRowKey}
        />
      {/if}
    </main>

    <footer>
      {#each snapshot.shortcut_footer as hint}
        <span><b>{hint.keys}</b> {hint.action === 'cycle_sort' ? `${snapshot.copy.filters.sort} ${activeSortLabel()}` : hint.label}</span>
      {/each}
    </footer>
  </div>

  {#if modal}
    <div class="scrim" role="presentation" use:fadeIn>
      <button class="backdrop" type="button" aria-label={snapshot.copy.actions.close_dialog} onclick={closeModal}></button>
      <section class="modal" role="dialog" aria-modal="true" tabindex="-1" use:reveal={{ y: 8 }}>
        <div class="modal-head">
          <div class="modal-title">
            {#if modal !== 'report'}<Search size={16} />{/if}
            {modalTitle(modal)}
          </div>
          <button class="icon-button" type="button" title={snapshot.copy.actions.close} onclick={closeModal}><X size={16} /></button>
        </div>

        {#if modal === 'project'}
          <input bind:value={query} placeholder={snapshot.copy.desktop.filter_projects} />
          <div class="picker-list">
            {#each filteredProjects() as project}
              <button
                type="button"
                class:selected={project.identity === snapshot.project.identity}
                onclick={() => commit(() => api.setProject(project.identity)).then(closeModal)}
              >
                <span>{project.label}</span>
                <small>{project.cost} · {count(project.calls)} {snapshot.copy.metrics.calls}</small>
              </button>
            {/each}
          </div>
        {:else if modal === 'session'}
          <input bind:value={query} placeholder={snapshot.copy.desktop.filter_sessions} />
          <div class="picker-list">
            {#each filteredSessions() as session}
              <button type="button" onclick={() => commit(() => api.openSession(session.key)).then(closeModal)}>
                <span>{session.project}</span>
                <small>{session.date} · {session.tool} · {session.cost} · {count(session.calls)} {snapshot.copy.metrics.calls}</small>
              </button>
            {/each}
          </div>
        {:else if modal === 'currency'}
          <input bind:value={query} placeholder={snapshot.copy.desktop.filter_currencies} />
          <div class="currency-grid">
            {#each filteredCurrencies() as currency}
              <button
                type="button"
                class:selected={currency === snapshot.currency}
                onclick={() => commit(() => api.setCurrency(currency)).then(closeModal)}
              >
                {currency}
              </button>
            {/each}
          </div>
        {:else if modal === 'advice_tool'}
          <input bind:value={query} placeholder={snapshot.copy.desktop.filter_advice_tools} />
          <div class="picker-list">
            {#each filteredAdviceTools() as tool}
              <button
                type="button"
                class:selected={tool.value === snapshot.advice_tool}
                onclick={() => commit(() => api.setAdviceTool(tool.value)).then(closeModal)}
              >
                <span>{tool.label}</span>
              </button>
            {/each}
          </div>
        {:else if modal === 'subscription_cookie'}
          <div class="cookie-modal">
            {#if cookieProvider === 'codex'}
              <p class="cookie-help">
                ChatGPT shards the NextAuth session token across two cookies. Copy each value from Dev&nbsp;Tools → Storage → Cookies on <em>chatgpt.com</em> and paste them below — both shards are required.
                {#if cookieIsSet(cookieProvider)}
                  A cookie set is already stored — leave the fields blank and use <em>Sync now</em>, or paste new values to replace them.
                {:else}
                  No cookies stored yet.
                {/if}
              </p>
              <div class="cookie-shards">
                <label class="cookie-shard">
                  <span><code>__Secure-next-auth.session-token.0</code></span>
                  <input
                    type="password"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Paste shard 0 value (~3–4 KB)"
                    bind:value={codexShard0}
                    disabled={cookieBusy}
                  />
                </label>
                <label class="cookie-shard">
                  <span><code>__Secure-next-auth.session-token.1</code></span>
                  <input
                    type="password"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Paste shard 1 value (~200 B)"
                    bind:value={codexShard1}
                    disabled={cookieBusy}
                  />
                </label>
                <label class="cookie-shard">
                  <span>Additional cookies <em>(optional — paste the full <code>Cookie:</code> header if Cloudflare or session-token-shards alone aren't enough)</em></span>
                  <textarea
                    autocomplete="off"
                    spellcheck="false"
                    rows="2"
                    placeholder="cf_clearance=…; __Host-next-auth.csrf-token=…"
                    bind:value={codexExtraCookies}
                    disabled={cookieBusy}
                  ></textarea>
                </label>
              </div>
            {:else}
              <p class="cookie-help">
                Paste the <code>sessionKey</code> cookie value from your {cookieProviderLabel(cookieProvider)} browser session.
                {#if cookieIsSet(cookieProvider)}
                  A cookie is already stored — leave the field blank and use <em>Sync now</em>, or paste a new value to replace it.
                {:else}
                  No cookie stored yet.
                {/if}
              </p>
              <input
                type="password"
                autocomplete="off"
                spellcheck="false"
                placeholder="Paste cookie value"
                bind:value={cookieValue}
                disabled={cookieBusy}
              />
            {/if}
            {#if cookieError}
              <div class="cookie-error">{cookieError}</div>
            {/if}
            <div class="cookie-actions">
              <button
                class="primary-command"
                type="button"
                disabled={cookieBusy || !cookieFormReady()}
                onclick={saveAndSyncCookie}
              >
                Save &amp; sync
              </button>
              <button
                type="button"
                disabled={cookieBusy || !cookieIsSet(cookieProvider)}
                onclick={syncWithStoredCookie}
              >
                Sync with stored cookie
              </button>
              <button
                class="danger"
                type="button"
                disabled={cookieBusy || !cookieIsSet(cookieProvider)}
                onclick={clearStoredCookie}
              >
                Clear stored cookie
              </button>
            </div>
            <p class="cookie-help muted">
              Stored locally in the OS keychain only.
              <a
                href={cookieProvider === 'codex'
                  ? 'https://github.com/russmckendrick/tokenuse/blob/main/docs/development/tools/codex-subscription.md'
                  : 'https://github.com/russmckendrick/tokenuse/blob/main/docs/development/tools/claude-subscription.md'}
                target="_blank"
                rel="noreferrer"
              >How to find your cookie</a>.
            </p>
          </div>
        {:else if modal === 'report'}
          <div class="export-box">
            <div class="export-path">{snapshot.report_dir}</div>
            <button type="button" onclick={chooseReportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
          </div>
          <div class="format-grid">
            {#each snapshot.periods as period}
              <button
                type="button"
                class:selected={period.value === reportPeriod}
                onclick={() => void selectReportPeriod(period.value)}
              >
                {period.label}
              </button>
            {/each}
          </div>
          <div class="export-box">
            <select bind:value={reportProjectIdentity} aria-label={snapshot.copy.reports.project}>
              {#each reportProjects as project}
                <option value={project.identity ?? ''}>{project.label}</option>
              {/each}
            </select>
            <label><input type="checkbox" bind:checked={reportRedacted} /> {snapshot.copy.reports.redaction}</label>
          </div>
          <div class="format-grid">
            {#each snapshot.report_formats as format}
              <button
                type="button"
                class:selected={format.value === reportFormat}
                onclick={() => (reportFormat = format.value)}
              >
                {format.label}
              </button>
            {/each}
          </div>
          <button class="primary-command" type="button" onclick={runReport}><Download size={16} /> {snapshot.copy.actions.export}</button>
        {/if}
      </section>
    </div>
  {/if}

  {#if callDetail}
    <div class="scrim" role="presentation" use:fadeIn>
      <button class="backdrop" type="button" aria-label={snapshot.copy.actions.close_call_detail} onclick={closeCallDetail}></button>
      <section class="modal detail-modal" role="dialog" aria-modal="true" tabindex="-1" use:reveal={{ y: 8 }}>
        <div class="modal-head">
          <div class="modal-title">{snapshot.copy.session.call_detail}</div>
          <button class="icon-button" type="button" title={snapshot.copy.actions.close} onclick={closeCallDetail}><X size={16} /></button>
        </div>

        <div class="detail-grid">
          <div><span>{snapshot.copy.tables.time}</span><strong>{callDetail.timestamp}</strong></div>
          <div><span>{snapshot.copy.tables.model}</span><strong>{callDetail.model}</strong></div>
          <div><span>{snapshot.copy.tables.cost}</span><strong class="money">{callDetail.cost}</strong></div>
          <div><span>{snapshot.copy.tables.tools}</span><strong>{callDetail.tools}</strong></div>
          <div><span>{snapshot.copy.metrics.input}</span><strong>{count(callDetail.input_tokens)}</strong></div>
          <div><span>{snapshot.copy.metrics.output}</span><strong>{count(callDetail.output_tokens)}</strong></div>
          <div><span>{snapshot.copy.metrics.cache_read}</span><strong>{count(callDetail.cache_read)}</strong></div>
          <div><span>{snapshot.copy.metrics.cache_write}</span><strong>{count(callDetail.cache_write)}</strong></div>
          <div><span>{snapshot.copy.metrics.cache_read_price}</span><strong>{callDetail.cache_read_rate}</strong></div>
          <div><span>{snapshot.copy.metrics.cache_write_price}</span><strong>{callDetail.cache_write_rate}</strong></div>
          <div><span>{snapshot.copy.session.reasoning}</span><strong>{count(callDetail.reasoning_tokens)}</strong></div>
          <div><span>{snapshot.copy.session.web_search}</span><strong>{count(callDetail.web_search_requests)}</strong></div>
        </div>

        {#if callDetail.bash_commands.length}
          <section class="detail-block">
            <h3>{snapshot.copy.session.bash}</h3>
            <pre>{callDetail.bash_commands.join('\n')}</pre>
          </section>
        {/if}

        <section class="detail-block">
          <h3>{snapshot.copy.tables.prompt}</h3>
          <pre>{callDetail.prompt_full || callDetail.prompt || '-'}</pre>
        </section>
      </section>
    </div>
  {/if}
{:else}
  <div class="loading" aria-busy="true" use:reveal></div>
{/if}
