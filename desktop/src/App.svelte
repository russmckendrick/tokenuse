<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { confirm, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { ArrowLeft, Database, Download, FolderOpen, RefreshCw, Search, Trash2, X } from 'lucide-svelte';
  import { api } from './api';
  import ActivityPulse from './components/ActivityPulse.svelte';
  import RankBar from './components/RankBar.svelte';
  import UsageConsole from './components/UsageConsole.svelte';
  import { fadeIn, reveal, staggeredReveal } from './motion';
  import Panel from './Panel.svelte';
  import TrayPopover from './TrayPopover.svelte';
  import type {
    ConfigRow,
    CountMetric,
    DesktopSnapshot,
    ExportFormatId,
    ModelMetric,
    PageId,
    ProjectMetric,
    ProjectOption,
    ProjectToolMetric,
    ShortcutInput,
    SortId,
    SessionDetail,
    SessionDetailView,
    SessionMetric,
    SessionOption,
    Summary,
    ToolId
  } from './types';

  type ModalKind = 'project' | 'session' | 'currency' | 'export' | null;

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
  let callDetail: SessionDetail | null = null;
  let query = '';
  let exportFormat: ExportFormatId = 'json';
  let clearingData = false;
  let pollTimer: number | undefined;

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
    exportFormat = snapshot?.export_formats[0]?.value ?? 'json';
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
        openModal('export');
        break;
      case 'close_modal':
        closeModal();
        break;
      case 'close_call_detail':
        closeCallDetail();
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

  async function chooseExportDir() {
    if (!snapshot) return;
    const selected = await openDialog({
      directory: true,
      multiple: false,
      defaultPath: snapshot.export_dir
    });
    if (typeof selected === 'string') {
      await commit(() => api.setExportDir(selected));
    }
  }

  async function runExport() {
    busy = true;
    error = null;
    try {
      const result = await api.exportCurrent(exportFormat);
      snapshot = result.snapshot;
      closeModal();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function count(value: number) {
    return value.toLocaleString();
  }

  function activePage() {
    return snapshot?.page ?? 'overview';
  }

  function activeSortLabel() {
    if (!snapshot) return '';
    return snapshot.sorts.find((sort) => sort.value === snapshot?.sort)?.label ?? '';
  }

  function tabsFor(state: DesktopSnapshot): Array<{ value: PageId; label: string }> {
    return [
      { value: 'overview', label: state.copy.nav.overview },
      { value: 'deep-dive', label: state.copy.nav.deep_dive },
      { value: 'usage', label: state.copy.nav.usage },
      { value: 'config', label: state.copy.nav.config }
    ];
  }

  function modalTitle(kind: Exclude<ModalKind, null>) {
    if (!snapshot) return kind;
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
      case 'clear_data':
        if (await confirmClearData()) {
          await runClearData();
        }
        break;
    }
  }

  async function confirmDownload(title: string, message: string) {
    try {
      return await confirm(message, {
        title,
        kind: 'warning',
        okLabel: snapshot?.copy.actions.download ?? '',
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
        <button class="icon-button" type="button" title={snapshot.copy.actions.refresh_archive} onclick={() => commit(() => api.refreshArchive())}>
          <RefreshCw size={16} />
        </button>
        <button class="icon-button" type="button" title={snapshot.copy.actions.export_current_view} onclick={() => openModal('export')}>
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
            onclick={() => commit(() => api.setPeriod(period.value))}
          >
            {period.label}
          </button>
        {/each}
      </div>

      <div class="filter-controls">
        <div class="segmented tool-strip" aria-label={snapshot.copy.desktop.tool_aria}>
          <span>{snapshot.copy.filters.tool}</span>
          <select aria-label={snapshot.copy.desktop.tool_aria} onchange={setToolFromEvent}>
            {#each snapshot.tools as tool}
              <option value={tool.value} selected={snapshot.tool === tool.value}>{tool.label}</option>
            {/each}
          </select>
        </div>

        <div class="segmented tool-strip sort-strip" aria-label={snapshot.copy.desktop.sort_aria}>
          <span>{snapshot.copy.filters.sort}</span>
          <select aria-label={snapshot.copy.desktop.sort_aria} onchange={setSortFromEvent}>
            {#each snapshot.sorts as sort}
              <option value={sort.value} selected={snapshot.sort === sort.value}>{sort.label}</option>
            {/each}
          </select>
        </div>

        <button class="segmented tool-strip project-strip" type="button" aria-label={snapshot.copy.desktop.project_aria} onclick={() => openModal('project')}>
          <span>{snapshot.copy.filters.project}</span>
          <strong>{snapshot.project.label}</strong>
        </button>
      </div>
    </section>

    {#if statusMessage()}
      <div
        class:error={statusTone() === 'error'}
        class:success={statusTone() === 'success'}
        class:warning={statusTone() === 'warning'}
        class:busy={statusTone() === 'busy'}
        class="status-line"
      >
        {statusMessage()}
      </div>
    {/if}

    <main>
      {#if activePage() === 'overview'}
        <section class="page overview-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
          {@render Kpis(snapshot.dashboard.summary, snapshot.currency)}
          <Panel title={snapshot.copy.panels.activity_pulse} tone="cyan">
            <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
          </Panel>
          <section class="grid overview-grid">
            <div class="overview-primary">
              <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow">
                {@render ProjectToolTable(snapshot.dashboard.project_tools)}
              </Panel>
            </div>
            <div class="overview-side-stack">
              <Panel title={snapshot.copy.panels.by_model} tone="magenta">
                {@render ModelTable(snapshot.dashboard.models)}
              </Panel>
              <Panel title={snapshot.copy.panels.shell_commands} tone="orange">
                {@render CountTable(snapshot.dashboard.commands)}
              </Panel>
              <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta">
                {@render CountTable(snapshot.dashboard.mcp_servers)}
              </Panel>
            </div>
          </section>
        </section>
      {:else if activePage() === 'deep-dive'}
        <section class="page deep-page" use:reveal={{ y: 5 }}>
          <section class="grid deep-grid">
            <div class="deep-trend">
              <Panel title={snapshot.copy.panels.activity_trend} tone="blue">
                <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
              </Panel>
            </div>
            <div class="deep-projects">
              <Panel title={snapshot.copy.panels.by_project} tone="green">
                {@render ProjectTable(snapshot.dashboard.projects)}
              </Panel>
            </div>
            <div class="deep-span">
              <Panel title={snapshot.copy.panels.top_sessions} tone="red">
                <button class="panel-command" type="button" onclick={() => openModal('session')}>{snapshot.copy.actions.open_session_picker}</button>
                {@render SessionTable(snapshot.dashboard.sessions)}
              </Panel>
            </div>
            <div class="deep-project-tools">
              <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow">
                {@render ProjectToolTable(snapshot.dashboard.project_tools)}
              </Panel>
            </div>
            <div class="deep-side-stack">
              <Panel title={snapshot.copy.panels.model_efficiency} tone="magenta">
                {@render ModelTable(snapshot.dashboard.models)}
              </Panel>
              <Panel title={snapshot.copy.panels.core_tools} tone="cyan">
                {@render CountTable(snapshot.dashboard.tools)}
              </Panel>
            </div>
            <div class="deep-shell">
              <Panel title={snapshot.copy.panels.shell_commands} tone="orange">
                {@render CountTable(snapshot.dashboard.commands)}
              </Panel>
            </div>
            <div class="deep-mcp">
              <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta">
                {@render CountTable(snapshot.dashboard.mcp_servers)}
              </Panel>
            </div>
          </section>
        </section>
      {:else if activePage() === 'usage'}
        <section class="page usage-page" use:reveal={{ y: 5 }}>
          <section class="usage-grid">
            {#each snapshot.usage.sections as section, index}
              <UsageConsole {section} tone={usageTone(section.tool, index)} copy={snapshot.copy} />
            {/each}
          </section>
        </section>
      {:else if activePage() === 'config'}
        <section class="page config-page" use:reveal={{ y: 5 }}>
          <section class="config-grid">
            <Panel title={snapshot.copy.nav.configuration} tone="cyan">
              <table>
                <thead>
                  <tr><th>{snapshot.copy.tables.setting}</th><th>{snapshot.copy.tables.value}</th><th></th></tr>
                </thead>
                <tbody>
                  {#each snapshot.config_rows as row}
                    <tr>
                      <td>{row.name}</td>
                      <td class="muted-cell">{row.value}</td>
                      <td class="tight">
                        <button class="row-action" class:danger={row.action === 'clear'} type="button" onclick={() => configAction(row)}>
                          {#if row.action === 'clear'}<Trash2 size={14} />{/if}
                          {row.action}
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </Panel>
            <Panel title={snapshot.copy.panels.desktop} tone="orange">
              <div class="toggle-list">
                <label class="toggle-row">
                  <span>
                    <strong>{snapshot.copy.desktop.open_at_login}</strong>
                    <small>{snapshot.desktop_settings.open_at_login ? snapshot.copy.desktop.enabled : snapshot.copy.desktop.disabled}</small>
                  </span>
                  <input
                    type="checkbox"
                    role="switch"
                    checked={snapshot.desktop_settings.open_at_login}
                    onchange={setOpenAtLoginFromEvent}
                  />
                  <i aria-hidden="true"></i>
                </label>
                <label class="toggle-row">
                  <span>
                    <strong>{snapshot.copy.desktop.dock_taskbar_icon}</strong>
                    <small>{snapshot.desktop_settings.show_dock_or_taskbar_icon ? snapshot.copy.desktop.shown : snapshot.copy.desktop.hidden}</small>
                  </span>
                  <input
                    type="checkbox"
                    role="switch"
                    checked={snapshot.desktop_settings.show_dock_or_taskbar_icon}
                    onchange={setShowDockOrTaskbarIconFromEvent}
                  />
                  <i aria-hidden="true"></i>
                </label>
              </div>
            </Panel>
            <Panel title={snapshot.copy.panels.local_data} tone="green">
              <div class="config-facts">
                <div><span>{snapshot.copy.tables.archive}</span><strong>{snapshot.source}</strong></div>
                <div><span>{snapshot.copy.tables.currency}</span><strong>{snapshot.currency}</strong></div>
                <div><span>{snapshot.copy.tables.exports}</span><strong>{snapshot.export_dir}</strong></div>
              </div>
              <div class="button-row">
                <button type="button" onclick={() => commit(() => api.refreshArchive())}><Database size={15} /> {snapshot.copy.actions.refresh}</button>
                <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
              </div>
            </Panel>
          </section>
        </section>
      {:else if activePage() === 'session'}
        {@render SessionDetailPanel(snapshot.session)}
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
            {#if modal !== 'export'}<Search size={16} />{/if}
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
        {:else if modal === 'export'}
          <div class="export-box">
            <div class="export-path">{snapshot.export_dir}</div>
            <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
          </div>
          <div class="format-grid">
            {#each snapshot.export_formats as format}
              <button
                type="button"
                class:selected={format.value === exportFormat}
                onclick={() => (exportFormat = format.value)}
              >
                {format.label}
              </button>
            {/each}
          </div>
          <button class="primary-command" type="button" onclick={runExport}><Download size={16} /> {snapshot.copy.actions.export}</button>
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

{#snippet Kpis(summary: Summary, currency: string)}
  <section class="kpis" use:staggeredReveal={{ selector: ':scope > div', y: 4, stagger: 0.025 }}>
    <div><span>{snapshot?.copy.metrics.cost}</span><strong>{summary.cost}</strong><small>{currency}</small></div>
    <div><span>{snapshot?.copy.metrics.calls}</span><strong>{summary.calls}</strong><small>{summary.input} {snapshot?.copy.metrics.in}</small></div>
    <div><span>{snapshot?.copy.metrics.sessions}</span><strong>{summary.sessions}</strong><small>{snapshot?.copy.metrics.active_set}</small></div>
    <div><span>{snapshot?.copy.metrics.cache_hit}</span><strong>{summary.cache_hit}</strong><small>{summary.cached} {snapshot?.copy.metrics.cached}</small></div>
    <div><span>{snapshot?.copy.metrics.in} / {snapshot?.copy.metrics.out}</span><strong>{summary.input}</strong><small>{summary.output} {snapshot?.copy.metrics.out}</small></div>
  </section>
{/snippet}

{#snippet ProjectTable(rows: ProjectMetric[])}
  <table class="data-table project-table">
    <thead><tr><th></th><th>{snapshot?.copy.tables.project}</th><th>{snapshot?.copy.tables.cost}</th><th>{snapshot?.copy.tables.avg_per_session}</th><th>{snapshot?.copy.tables.sess}</th><th>{snapshot?.copy.tables.tools}</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} ${snapshot?.copy.desktop.rank}`} /></td>
          <td>{row.name}</td>
          <td class="money">{row.cost}</td>
          <td class="money">{row.avg_per_session}</td>
          <td>{count(row.sessions)}</td>
          <td class="muted-cell">{row.tool_mix}</td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty-cell">{snapshot?.copy.empty.no_project_rows}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ProjectToolTable(rows: ProjectToolMetric[])}
  <table class="data-table project-tool-table">
    <thead><tr><th></th><th>{snapshot?.copy.tables.project}</th><th>{snapshot?.copy.tables.tool}</th><th>{snapshot?.copy.tables.cost}</th><th>{snapshot?.copy.tables.calls}</th><th>{snapshot?.copy.tables.sess}</th><th>{snapshot?.copy.tables.avg_per_session}</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.project} ${row.tool} ${snapshot?.copy.desktop.rank}`} /></td>
          <td>{row.project}</td>
          <td>{row.tool}</td>
          <td class="money">{row.cost}</td>
          <td>{count(row.calls)}</td>
          <td>{count(row.sessions)}</td>
          <td class="money">{row.avg_per_session}</td>
        </tr>
      {:else}
        <tr><td colspan="7" class="empty-cell">{snapshot?.copy.empty.no_project_tool_rows}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet SessionTable(rows: SessionMetric[])}
  <table class="data-table session-table">
    <thead><tr><th></th><th>{snapshot?.copy.tables.date}</th><th>{snapshot?.copy.tables.project}</th><th>{snapshot?.copy.tables.cost}</th><th>{snapshot?.copy.tables.calls}</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.project} ${snapshot?.copy.desktop.session_rank}`} /></td>
          <td>{row.date}</td>
          <td>{row.project}</td>
          <td class="money">{row.cost}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="empty-cell">{snapshot?.copy.empty.no_sessions}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ModelTable(rows: ModelMetric[])}
  <table class="data-table model-table">
    <thead><tr><th></th><th>{snapshot?.copy.tables.model}</th><th>{snapshot?.copy.tables.cost}</th><th>{snapshot?.copy.tables.cache}</th><th>{snapshot?.copy.tables.calls}</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} ${snapshot?.copy.desktop.rank}`} /></td>
          <td>{row.name}</td>
          <td class="money">{row.cost}</td>
          <td>{row.cache}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="empty-cell">{snapshot?.copy.empty.no_models}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet CountTable(rows: CountMetric[])}
  <table class="data-table count-table">
    <thead><tr><th></th><th>{snapshot?.copy.tables.name}</th><th>{snapshot?.copy.tables.calls}</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} ${snapshot?.copy.desktop.rank}`} /></td>
          <td>{row.name}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="3" class="empty-cell">{snapshot?.copy.empty.no_rows}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet SessionDetailPanel(session: SessionDetailView | null)}
  <section class="session-page" use:reveal={{ y: 5 }}>
    <div class="session-head">
      <button type="button" onclick={() => commit(() => api.closeSession())}><ArrowLeft size={15} /> {snapshot?.copy.nav.deep_dive}</button>
      {#if session}
        <div>
          <strong>{session.project}</strong>
          <span>{session.tool} · {session.date_range}</span>
        </div>
      {/if}
    </div>
    {#if session}
      <section class="kpis session-kpis">
        <div><span>{snapshot?.copy.metrics.cost}</span><strong>{session.total_cost}</strong><small>{session.total_calls} {snapshot?.copy.metrics.calls}</small></div>
        <div><span>{snapshot?.copy.metrics.input}</span><strong>{session.total_input}</strong><small>{snapshot?.copy.metrics.tokens}</small></div>
        <div><span>{snapshot?.copy.metrics.output}</span><strong>{session.total_output}</strong><small>{snapshot?.copy.metrics.tokens}</small></div>
        <div><span>{snapshot?.copy.metrics.cache_read}</span><strong>{session.total_cache_read}</strong><small>{snapshot?.copy.metrics.tokens}</small></div>
      </section>
      <div class="session-panel-area">
        {#if session.note}<div class="status-line">{session.note}</div>{/if}
        <Panel title={snapshot?.copy.panels.calls ?? ''} tone="red">
          <table>
            <thead><tr><th>{snapshot?.copy.tables.time}</th><th>{snapshot?.copy.tables.model}</th><th>{snapshot?.copy.tables.cost}</th><th>{snapshot?.copy.tables.in}</th><th>{snapshot?.copy.tables.out}</th><th>{snapshot?.copy.tables.cache}</th><th>{snapshot?.copy.tables.tools}</th><th>{snapshot?.copy.tables.prompt}</th></tr></thead>
            <tbody>
              {#each session.calls as call}
                <tr
                  class="click-row"
                  tabindex="0"
                  onclick={() => openCallDetail(call)}
                  onkeydown={(event) => handleCallRowKey(event, call)}
                >
                  <td>{call.timestamp}</td>
                  <td>{call.model}</td>
                  <td class="money">{call.cost}</td>
                  <td>{count(call.input_tokens)}</td>
                  <td>{count(call.output_tokens)}</td>
                  <td>{count(call.cache_read + call.cache_write)}</td>
                  <td>{call.tools}</td>
                  <td class="prompt-cell">{call.prompt}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </Panel>
      </div>
    {:else}
      <div class="session-panel-area">
        <div class="empty-state">{snapshot?.copy.session.no_session_selected}</div>
      </div>
    {/if}
  </section>
{/snippet}
