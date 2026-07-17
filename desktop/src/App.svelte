<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fly } from 'svelte/transition';
  import { Channel } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { confirm, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { Download, FolderOpen, Search, X } from 'lucide-svelte';
  import { api } from './api';
  import { count } from './format';
  import { router, type Route } from './lib/router.svelte';
  import { resolveShortcut } from './lib/shortcuts';
  import { fadeIn, pageTransition, reveal } from './motion';
  import PageHeader from './shell/PageHeader.svelte';
  import Sidebar from './shell/Sidebar.svelte';
  import StatusBar from './shell/StatusBar.svelte';
  import TrayPopover from './TrayPopover.svelte';
  import CoachPage from './routes/CoachPage.svelte';
  import ModelsPage from './routes/ModelsPage.svelte';
  import OverviewPage from './routes/OverviewPage.svelte';
  import ProjectsPage from './routes/ProjectsPage.svelte';
  import ToolsPage from './routes/ToolsPage.svelte';
  import AnalyticsPage from './routes/AnalyticsPage.svelte';
  import ConfigView from './views/ConfigView.svelte';
  import ProjectPage from './routes/ProjectPage.svelte';
  import SessionView from './views/SessionView.svelte';
  import type {
    ConfigRow,
    DesktopSnapshot,
    DesktopUpdateDownloadEvent,
    DesktopUpdateMetadata,
    PeriodId,
    ProjectOption,
    ReportFormatId,
    SessionDetail,
    SessionDetailView,
    SessionOption,
    ShortcutHint,
    SortId,
    ToolId
  } from './types';

  type ModalKind = 'project' | 'session' | 'currency' | 'report' | 'subscription_cookie' | null;
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

  const SIDEBAR_COLLAPSED_KEY = 'tokenuse.sidebar.collapsed';

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
  let callDetail: SessionDetail | null = null;
  let sessionDetail: SessionDetailView | null = null;
  let query = '';
  let reportFormat: ReportFormatId = 'html';
  let reportPeriod: PeriodId = 'week';
  let reportProjectIdentity = '';
  let reportProjects: ProjectOption[] = [];
  let reportRedacted = false;
  let clearingData = false;
  let pollTimer: number | undefined;
  let toastTimer: number | undefined;
  let sidebarCollapsed = false;
  let toastMessage: string | null = null;
  let toastTone = 'info';
  let observedError: string | null = null;
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

    sidebarCollapsed = localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === '1';
    void load();
    pollTimer = window.setInterval(() => void loadSilent(), 3000);
    window.addEventListener('keydown', handleKey);

    return () => {
      if (pollTimer !== undefined) {
        window.clearInterval(pollTimer);
      }
      if (toastTimer !== undefined) {
        window.clearTimeout(toastTimer);
      }
      window.removeEventListener('keydown', handleKey);
    };
  });

  async function load() {
    await commit(() => api.snapshot());
  }

  async function loadSilent() {
    try {
      const previousStatus = snapshot?.status ?? null;
      const next = await api.snapshot();
      snapshot = next;
      if (next.status && next.status !== previousStatus) {
        showStatusToast(next.status, next.status_tone);
      }
    } catch {
      // Keep the last good render during transient backend errors.
    }
  }

  async function commit(action: () => Promise<DesktopSnapshot>) {
    busy = true;
    error = null;
    try {
      const next = await action();
      snapshot = next;
      if (next.status) showStatusToast(next.status, next.status_tone);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function navigate(next: Route) {
    cancelScrollRestore();
    router.go(next);
  }

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, sidebarCollapsed ? '1' : '0');
  }

  function pageTitle(route: Route, currentSnapshot: DesktopSnapshot | null): string {
    if (!currentSnapshot) return '';
    const nav = currentSnapshot.copy.nav;
    switch (route.page) {
      case 'overview':
        return nav.overview;
      case 'analytics':
        return nav.analytics;
      case 'coach':
        return nav.coach;
      case 'tools': {
        if (route.tool) {
          const tool = currentSnapshot.tools.find((t) => t.value === route.tool);
          if (tool) return tool.label;
        }
        return nav.tools;
      }
      case 'models':
        return nav.models;
      case 'projects':
        return route.project?.label ?? nav.projects;
      case 'config':
        return nav.config;
      case 'session':
        return nav.session;
    }
  }

  $: currentPageTitle = pageTitle(router.route, snapshot);

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

  /** Where the session view returns to on close: the page it was opened from. */
  let sessionReturn: Route = { page: 'analytics' };
  let sessionReturnScroll = 0;
  let sessionReturnHeight = 0;
  let pageScrollEl: HTMLElement | null = null;
  let scrollRestoreObserver: ResizeObserver | null = null;

  function cancelScrollRestore() {
    scrollRestoreObserver?.disconnect();
    scrollRestoreObserver = null;
  }

  /** The return page loads parts of its content async. Applying the saved
   * offset against a partially-built layout would pin the wrong content (and
   * scroll anchoring then keeps it wrong), so wait until the page has grown
   * back to the height the offset was saved against. */
  function restorePageScroll(top: number, height: number) {
    cancelScrollRestore();
    const el = pageScrollEl;
    if (!el) return;
    const apply = () => {
      if (el.scrollHeight < height - 8) return false;
      el.scrollTop = top;
      return true;
    };
    if (apply()) return;
    const content = el.firstElementChild;
    if (!content) return;
    const observer = new ResizeObserver(() => {
      if (apply() && scrollRestoreObserver === observer) cancelScrollRestore();
    });
    observer.observe(content);
    scrollRestoreObserver = observer;
  }

  async function openSession(key: string) {
    busy = true;
    error = null;
    try {
      sessionDetail = await api.getSessionDetail(key);
      if (router.route.page !== 'session') {
        sessionReturn = { ...router.route };
        sessionReturnScroll = pageScrollEl?.scrollTop ?? 0;
        sessionReturnHeight = pageScrollEl?.scrollHeight ?? 0;
      }
      navigate({ page: 'session', sessionKey: key });
      await tick();
      pageScrollEl?.scrollTo({ top: 0 });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function closeSession() {
    sessionDetail = null;
    navigate(sessionReturn);
    await tick();
    restorePageScroll(sessionReturnScroll, sessionReturnHeight);
  }

  /** Open a project's page from a ranked-table row, which only carries the
   * display label; resolve the identity via the snapshot's project options. */
  async function openProject(name: string) {
    const option = snapshot?.projects.find((project) => project.identity !== null && project.label === name);
    await openProjectPage(option?.identity ?? null, name);
  }

  async function openProjectPage(identity: string | null, label: string) {
    navigate(identity ? { page: 'projects', project: { identity, label } } : { page: 'projects' });
    await tick();
    pageScrollEl?.scrollTo({ top: 0 });
  }

  /** Client-side navigation keys; everything else falls through to the shared keymap. */
  function clientNavTarget(event: KeyboardEvent): Route | null {
    if (event.ctrlKey || event.altKey || event.metaKey) return null;
    if (event.key === 'Tab') {
      router.cycle(event.shiftKey ? -1 : 1);
      return router.route;
    }
    switch (event.key) {
      case 'o':
        return { page: 'overview' };
      case 'd':
        return { page: 'analytics' };
      case 'h':
        return { page: 'coach' };
      case 'u':
        return { page: 'tools' };
      case 'c':
        return { page: 'config' };
      default:
        return null;
    }
  }

  function handleKey(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (
      (target?.tagName === 'INPUT' || target?.tagName === 'SELECT' || target?.tagName === 'TEXTAREA') &&
      event.key !== 'Escape'
    ) {
      return;
    }
    if (!snapshot) return;

    const action = resolveShortcut(event);

    if (action?.kind === 'escape') {
      event.preventDefault();
      if (callDetail) closeCallDetail();
      else if (modal) closeModal();
      else if (router.route.page === 'session') closeSession();
      return;
    }

    if (modal || callDetail) return;

    if (router.route.page !== 'session') {
      const targetRoute = clientNavTarget(event);
      if (targetRoute) {
        event.preventDefault();
        navigate(targetRoute);
        return;
      }
    }

    if (!action) return;
    const page = router.route.page;
    const periodLocked = page === 'tools' && router.route.tool === undefined;
    const dataFiltersActive = page === 'overview' || page === 'analytics';

    switch (action.kind) {
      case 'period':
        if (page === 'config' || periodLocked) return;
        event.preventDefault();
        setPeriod(action.period);
        return;
      case 'cycle-tool': {
        if (!dataFiltersActive) return;
        event.preventDefault();
        const tools = snapshot.tools;
        const idx = tools.findIndex((tool) => tool.value === snapshot?.tool);
        const next = tools[(idx + 1 + tools.length) % tools.length];
        void commit(() => api.setTool(next.value));
        return;
      }
      case 'cycle-sort': {
        if (page === 'config') return;
        event.preventDefault();
        const sorts = snapshot.sorts;
        const idx = sorts.findIndex((sort) => sort.value === snapshot?.sort);
        const next = sorts[(idx + 1 + sorts.length) % sorts.length];
        void commit(() => api.setSort(next.value));
        return;
      }
      case 'toggle-source':
        event.preventDefault();
        void commit(() => api.toggleDataSource());
        return;
      case 'open-project-picker':
        if (!dataFiltersActive && page !== 'projects') return;
        event.preventDefault();
        openModal('project');
        return;
      case 'open-session-picker':
        event.preventDefault();
        openModal('session');
        return;
      case 'open-report':
        event.preventDefault();
        openModal('report');
        return;
      case 'refresh':
        event.preventDefault();
        void commit(() => api.refreshArchive());
        return;
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

  function setPeriod(period: PeriodId) {
    void commit(() => api.setPeriod(period));
  }

  function setOpenAtLoginFromEvent(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void commit(() => api.setOpenAtLogin(enabled));
  }

  function setShowDockOrTaskbarIconFromEvent(event: Event) {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void commit(() => api.setShowDockOrTaskbarIcon(enabled));
  }

  function setPlanPriceFromEvent(id: string, event: Event) {
    const raw = (event.currentTarget as HTMLInputElement).value.trim();
    const parsed = raw === '' ? Number.NaN : Number(raw);
    const price = Number.isFinite(parsed) && parsed > 0 ? parsed : null;
    void commit(() => api.setPlanPrice(id, price));
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

  function activeSortLabel() {
    if (!snapshot) return '';
    return snapshot.sorts.find((sort) => sort.value === snapshot?.sort)?.label ?? '';
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

  function showStatusToast(message: string, tone: string) {
    if (toastTimer !== undefined) window.clearTimeout(toastTimer);
    toastMessage = message;
    toastTone = tone;
    toastTimer = window.setTimeout(
      () => {
        toastMessage = null;
        toastTimer = undefined;
      },
      tone === 'error' ? 8000 : 4200
    );
  }

  $: {
    if (!error) {
      observedError = null;
    } else if (error !== observedError) {
      observedError = error;
      showStatusToast(error, 'error');
    }
  }

  function statusHints(): ShortcutHint[] {
    if (!snapshot) return [];
    const footers = snapshot.copy.keymap.footers;
    const name =
      router.route.page === 'tools'
        ? 'desktop_usage'
        : router.route.page === 'config'
          ? 'desktop_config'
          : 'desktop';
    return footers[name] ?? footers['desktop'] ?? [];
  }
</script>

{#if isTrayPopover}
  <TrayPopover />
{:else if snapshot}
  <div class="app-shell" class:is-busy={busy} class:sidebar-collapsed={sidebarCollapsed}>
    <Sidebar
      copy={snapshot.copy}
      route={router.route}
      tools={snapshot.tools}
      usageOrder={snapshot.usage.sections
        .slice()
        .sort((left, right) => right.usage.calls - left.usage.calls || left.tool.localeCompare(right.tool))
        .map((section) => section.tool)}
      collapsed={sidebarCollapsed}
      {navigate}
      toggleCollapsed={toggleSidebar}
    />

    <div class="content">
      <PageHeader
        copy={snapshot.copy}
        title={currentPageTitle}
        {snapshot}
        showPeriod={router.route.page !== 'config' && router.route.page !== 'session' && (router.route.page !== 'tools' || router.route.tool !== undefined)}
        showTool={router.route.page === 'overview' || router.route.page === 'analytics' || router.route.page === 'coach'}
        showSort={router.route.page !== 'config' && router.route.page !== 'session' && router.route.page !== 'models' && router.route.page !== 'coach'}
        showProject={router.route.page === 'overview' || router.route.page === 'analytics' || router.route.page === 'coach' || (router.route.page === 'projects' && !router.route.project)}
        {setPeriod}
        setTool={setToolFromEvent}
        setSort={setSortFromEvent}
        openProjectPicker={() => openModal('project')}
        refresh={() => commit(() => api.refreshArchive())}
        openReport={() => openModal('report')}
      />

      <main class="page-scroll" bind:this={pageScrollEl}>
        {#key `${router.route.page}:${router.route.tool ?? ''}:${router.route.project?.identity ?? ''}`}
          <div class="route-view" use:pageTransition>
            {#if router.route.page === 'overview'}
              <OverviewPage {snapshot} {openProject} />
            {:else if router.route.page === 'analytics'}
              <AnalyticsPage {snapshot} openSessionPicker={() => openModal('session')} {openSession} {openProject} />
            {:else if router.route.page === 'coach'}
              <CoachPage {snapshot} />
            {:else if router.route.page === 'tools'}
              <ToolsPage {snapshot} tool={router.route.tool} {usageTone} {navigate} {openSession} {openProject} />
            {:else if router.route.page === 'models'}
              <ModelsPage {snapshot} />
            {:else if router.route.page === 'projects'}
              {#if router.route.project}
                <ProjectPage {snapshot} project={router.route.project} {openSession} />
              {:else}
                <ProjectsPage {snapshot} {openProjectPage} />
              {/if}
            {:else if router.route.page === 'config'}
              <ConfigView
                {snapshot}
                {configAction}
                chooseExportDir={chooseReportDir}
                refreshArchive={() => commit(() => api.refreshArchive())}
                {desktopUpdate}
                checkDesktopUpdate={() => void checkDesktopUpdate()}
                installDesktopUpdate={() => void installDesktopUpdate()}
                toggleSampleData={() => void commit(() => api.toggleDataSource())}
                {setOpenAtLoginFromEvent}
                {setShowDockOrTaskbarIconFromEvent}
                {setPlanPriceFromEvent}
              />
            {:else if router.route.page === 'session'}
              <SessionView
                {snapshot}
                session={sessionDetail}
                backLabel={pageTitle(sessionReturn, snapshot)}
                {closeSession}
                {openCallDetail}
                {handleCallRowKey}
              />
            {/if}
          </div>
        {/key}
      </main>

      <StatusBar
        copy={snapshot.copy}
        source={snapshot.source}
        currency={snapshot.currency}
        hints={statusHints()}
        sortLabel={activeSortLabel()}
      />
    </div>

    {#if toastMessage}
      <div
        class="status-toast"
        class:error={toastTone === 'error'}
        class:success={toastTone === 'success'}
        class:warning={toastTone === 'warning'}
        class:busy={toastTone === 'busy'}
        role="status"
        aria-live="polite"
        in:fly={{ y: 6, duration: 180 }}
        out:fly={{ y: 6, duration: 360 }}
      >
        <i class="status-dot" aria-hidden="true"></i>
        <span>{toastMessage}</span>
      </div>
    {/if}
  </div>

  {#if modal}
    <div class="scrim" role="presentation" use:fadeIn>
      <button class="backdrop" type="button" aria-label={snapshot.copy.actions.close_dialog} onclick={closeModal}></button>
      <section class="modal" role="dialog" aria-modal="true" tabindex="-1" use:reveal={{ y: 8 }}>
        <div class="modal-head">
          <div class="modal-title">
            {#if modal !== 'report'}<Search size={16} />{/if}
            {modal === 'report'
              ? snapshot.copy.reports.modal_title
              : modal === 'subscription_cookie'
                ? (cookieProvider === 'codex'
                    ? snapshot.copy.modals.sync_codex_subscription_limits_title
                    : snapshot.copy.modals.sync_claude_subscription_limits_title)
                : snapshot.copy.modals[modal] ?? modal}
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
              <button type="button" onclick={() => { closeModal(); void openSession(session.key); }}>
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
          <div><span>{snapshot.copy.session.interaction_mode}</span><strong>{callDetail.interaction_mode}</strong></div>
          <div><span>{snapshot.copy.session.token_quality}</span><strong>{callDetail.token_quality}</strong></div>
          <div><span>{snapshot.copy.session.timestamp_quality}</span><strong>{callDetail.timestamp_quality}</strong></div>
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
{:else if error}
  <div class="loading startup-error" role="alert">{error}</div>
{:else}
  <div class="loading" aria-busy="true" use:reveal></div>
{/if}
