<script lang="ts">
  import { onMount } from 'svelte';
  import { confirm, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { ArrowLeft, Database, Download, FolderOpen, RefreshCw, Search, X } from 'lucide-svelte';
  import { api } from './api';
  import ActivityPulse from './components/ActivityPulse.svelte';
  import RankBar from './components/RankBar.svelte';
  import UsageConsole from './components/UsageConsole.svelte';
  import Panel from './Panel.svelte';
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

  const tabs: Array<{ value: PageId; label: string }> = [
    { value: 'overview', label: 'Overview' },
    { value: 'deep-dive', label: 'Deep Dive' },
    { value: 'usage', label: 'Usage' },
    { value: 'config', label: 'Config' }
  ];

  let snapshot: DesktopSnapshot | null = null;
  let busy = false;
  let error: string | null = null;
  let modal: ModalKind = null;
  let callDetail: SessionDetail | null = null;
  let query = '';
  let exportFormat: ExportFormatId = 'json';
  let pollTimer: number | undefined;

  onMount(() => {
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

  function usageTone(tool: string, index: number) {
    const normalized = tool.toLowerCase();
    if (normalized.includes('codex')) return 'orange';
    if (normalized.includes('claude')) return 'magenta';
    if (normalized.includes('cursor')) return 'blue';
    if (normalized.includes('copilot')) return 'green';
    return ['cyan', 'yellow', 'magenta', 'green'][index % 4];
  }

  function configAction(row: ConfigRow) {
    void runConfigAction(row);
  }

  async function runConfigAction(row: ConfigRow) {
    if (row.name === 'currency override') {
      openModal('currency');
    } else if (row.name === 'rates.json') {
      const confirmed = await confirmDownload(
        'Download rates.json?',
        'This will download the latest published tokenuse currency snapshot into your local config directory.'
      );
      if (confirmed) {
        await commit(() => api.refreshCurrencyRates());
      }
    } else if (row.name === 'LiteLLM prices') {
      const confirmed = await confirmDownload(
        'Download LiteLLM prices?',
        'This will download the latest LiteLLM model price table into your local config directory.'
      );
      if (confirmed) {
        await commit(() => api.refreshPricingSnapshot());
      }
    }
  }

  async function confirmDownload(title: string, message: string) {
    try {
      return await confirm(message, {
        title,
        kind: 'warning',
        okLabel: 'Download',
        cancelLabel: 'Cancel'
      });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      return false;
    }
  }
</script>

{#if snapshot}
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
        <span class="brand-title">Token Use</span>
      </div>

      <nav class="tabs" aria-label="Sections">
        {#each tabs as tab}
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
        <button class="icon-button" type="button" title="Refresh archive" onclick={() => commit(() => api.refreshArchive())}>
          <RefreshCw size={16} />
        </button>
        <button class="icon-button" type="button" title="Export current view" onclick={() => openModal('export')}>
          <Download size={16} />
        </button>
      </div>
    </header>

    <section class="filter-strip">
      <div class="segmented" aria-label="Period">
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

      <div class="segmented tool-strip" aria-label="Tool">
        <span>tool</span>
        <select aria-label="Tool" onchange={setToolFromEvent}>
          {#each snapshot.tools as tool}
            <option value={tool.value} selected={snapshot.tool === tool.value}>{tool.label}</option>
          {/each}
        </select>
      </div>

      <div class="segmented tool-strip sort-strip" aria-label="Sort">
        <span>sort</span>
        <select aria-label="Sort" onchange={setSortFromEvent}>
          {#each snapshot.sorts as sort}
            <option value={sort.value} selected={snapshot.sort === sort.value}>{sort.label}</option>
          {/each}
        </select>
      </div>

      <button class="project-pill" type="button" onclick={() => openModal('project')}>
        <span>project</span>
        <strong>{snapshot.project.label}</strong>
      </button>
    </section>

    {#if snapshot.status || error}
      <div class:error={Boolean(error)} class="status-line">
        {error ?? snapshot.status}
      </div>
    {/if}

    <main>
      {#if activePage() === 'overview'}
        <section class="page overview-page">
          {@render Kpis(snapshot.dashboard.summary, snapshot.currency)}
          <Panel title="Activity Pulse" tone="cyan">
            <ActivityPulse points={snapshot.dashboard.activity_timeline} />
          </Panel>
          <section class="grid overview-grid">
            <div class="overview-primary">
              <Panel title="Project Spend by Tool" tone="yellow">
                {@render ProjectToolTable(snapshot.dashboard.project_tools)}
              </Panel>
            </div>
            <div class="overview-side-stack">
              <Panel title="By Model" tone="magenta">
                {@render ModelTable(snapshot.dashboard.models)}
              </Panel>
              <Panel title="Shell Commands" tone="orange">
                {@render CountTable(snapshot.dashboard.commands)}
              </Panel>
              <Panel title="MCP Servers" tone="magenta">
                {@render CountTable(snapshot.dashboard.mcp_servers)}
              </Panel>
            </div>
          </section>
        </section>
      {:else if activePage() === 'deep-dive'}
        <section class="page deep-page">
          <section class="grid deep-grid">
            <div class="deep-trend">
              <Panel title="Activity Trend" tone="blue">
                <ActivityPulse points={snapshot.dashboard.activity_timeline} />
              </Panel>
            </div>
            <div class="deep-projects">
              <Panel title="By Project" tone="green">
                {@render ProjectTable(snapshot.dashboard.projects)}
              </Panel>
            </div>
            <div class="deep-span">
              <Panel title="Top Sessions" tone="red">
                <button class="panel-command" type="button" onclick={() => openModal('session')}>Open Session Picker</button>
                {@render SessionTable(snapshot.dashboard.sessions)}
              </Panel>
            </div>
            <div class="deep-project-tools">
              <Panel title="Project Spend by Tool" tone="yellow">
                {@render ProjectToolTable(snapshot.dashboard.project_tools)}
              </Panel>
            </div>
            <div class="deep-side-stack">
              <Panel title="Model Efficiency" tone="magenta">
                {@render ModelTable(snapshot.dashboard.models)}
              </Panel>
              <Panel title="Core Tools" tone="cyan">
                {@render CountTable(snapshot.dashboard.tools)}
              </Panel>
            </div>
            <div class="deep-shell">
              <Panel title="Shell Commands" tone="orange">
                {@render CountTable(snapshot.dashboard.commands)}
              </Panel>
            </div>
            <div class="deep-mcp">
              <Panel title="MCP Servers" tone="magenta">
                {@render CountTable(snapshot.dashboard.mcp_servers)}
              </Panel>
            </div>
          </section>
        </section>
      {:else if activePage() === 'usage'}
        <section class="page usage-page">
          <section class="usage-grid">
            {#each snapshot.usage.sections as section, index}
              <UsageConsole {section} tone={usageTone(section.tool, index)} />
            {/each}
          </section>
        </section>
      {:else if activePage() === 'config'}
        <section class="page config-page">
          <section class="config-grid">
            <Panel title="Configuration" tone="cyan">
              <table>
                <thead>
                  <tr><th>setting</th><th>value</th><th></th></tr>
                </thead>
                <tbody>
                  {#each snapshot.config_rows as row}
                    <tr>
                      <td>{row.name}</td>
                      <td class="muted-cell">{row.value}</td>
                      <td class="tight">
                        <button class="row-action" type="button" onclick={() => configAction(row)}>{row.action}</button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </Panel>
            <Panel title="Local Data" tone="green">
              <div class="config-facts">
                <div><span>archive</span><strong>{snapshot.source}</strong></div>
                <div><span>currency</span><strong>{snapshot.currency}</strong></div>
                <div><span>exports</span><strong>{snapshot.export_dir}</strong></div>
              </div>
              <div class="button-row">
                <button type="button" onclick={() => commit(() => api.refreshArchive())}><Database size={15} /> Refresh</button>
                <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> Folder</button>
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
        <span><b>{hint.keys}</b> {hint.action === 'cycle_sort' ? `sort ${activeSortLabel()}` : hint.label}</span>
      {/each}
    </footer>
  </div>

  {#if modal}
    <div class="scrim" role="presentation">
      <button class="backdrop" type="button" aria-label="Close dialog" onclick={closeModal}></button>
      <section class="modal" role="dialog" aria-modal="true" tabindex="-1">
        <div class="modal-head">
          <div class="modal-title">
            {#if modal !== 'export'}<Search size={16} />{/if}
            {modal}
          </div>
          <button class="icon-button" type="button" title="Close" onclick={closeModal}><X size={16} /></button>
        </div>

        {#if modal === 'project'}
          <input bind:value={query} placeholder="Filter projects" />
          <div class="picker-list">
            {#each filteredProjects() as project}
              <button
                type="button"
                class:selected={project.identity === snapshot.project.identity}
                onclick={() => commit(() => api.setProject(project.identity)).then(closeModal)}
              >
                <span>{project.label}</span>
                <small>{project.cost} · {count(project.calls)} calls</small>
              </button>
            {/each}
          </div>
        {:else if modal === 'session'}
          <input bind:value={query} placeholder="Filter sessions" />
          <div class="picker-list">
            {#each filteredSessions() as session}
              <button type="button" onclick={() => commit(() => api.openSession(session.key)).then(closeModal)}>
                <span>{session.project}</span>
                <small>{session.date} · {session.tool} · {session.cost} · {count(session.calls)} calls</small>
              </button>
            {/each}
          </div>
        {:else if modal === 'currency'}
          <input bind:value={query} placeholder="Filter currencies" />
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
            <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> Folder</button>
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
          <button class="primary-command" type="button" onclick={runExport}><Download size={16} /> Export</button>
        {/if}
      </section>
    </div>
  {/if}

  {#if callDetail}
    <div class="scrim" role="presentation">
      <button class="backdrop" type="button" aria-label="Close call detail" onclick={closeCallDetail}></button>
      <section class="modal detail-modal" role="dialog" aria-modal="true" tabindex="-1">
        <div class="modal-head">
          <div class="modal-title">call detail</div>
          <button class="icon-button" type="button" title="Close" onclick={closeCallDetail}><X size={16} /></button>
        </div>

        <div class="detail-grid">
          <div><span>time</span><strong>{callDetail.timestamp}</strong></div>
          <div><span>model</span><strong>{callDetail.model}</strong></div>
          <div><span>cost</span><strong class="money">{callDetail.cost}</strong></div>
          <div><span>tools</span><strong>{callDetail.tools}</strong></div>
          <div><span>input</span><strong>{count(callDetail.input_tokens)}</strong></div>
          <div><span>output</span><strong>{count(callDetail.output_tokens)}</strong></div>
          <div><span>cache read</span><strong>{count(callDetail.cache_read)}</strong></div>
          <div><span>cache write</span><strong>{count(callDetail.cache_write)}</strong></div>
          <div><span>reasoning</span><strong>{count(callDetail.reasoning_tokens)}</strong></div>
          <div><span>web search</span><strong>{count(callDetail.web_search_requests)}</strong></div>
        </div>

        {#if callDetail.bash_commands.length}
          <section class="detail-block">
            <h3>bash</h3>
            <pre>{callDetail.bash_commands.join('\n')}</pre>
          </section>
        {/if}

        <section class="detail-block">
          <h3>prompt</h3>
          <pre>{callDetail.prompt_full || callDetail.prompt || '-'}</pre>
        </section>
      </section>
    </div>
  {/if}
{:else}
  <div class="loading">Token Use</div>
{/if}

{#snippet Kpis(summary: Summary, currency: string)}
  <section class="kpis">
    <div><span>cost</span><strong>{summary.cost}</strong><small>{currency}</small></div>
    <div><span>calls</span><strong>{summary.calls}</strong><small>{summary.input} in</small></div>
    <div><span>sessions</span><strong>{summary.sessions}</strong><small>active set</small></div>
    <div><span>cache hit</span><strong>{summary.cache_hit}</strong><small>{summary.cached} cached</small></div>
    <div><span>in / out</span><strong>{summary.input}</strong><small>{summary.output} out</small></div>
  </section>
{/snippet}

{#snippet ProjectTable(rows: ProjectMetric[])}
  <table class="data-table project-table">
    <thead><tr><th></th><th>project</th><th>cost</th><th>avg/s</th><th>sess</th><th>tools</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} rank`} /></td>
          <td>{row.name}</td>
          <td class="money">{row.cost}</td>
          <td class="money">{row.avg_per_session}</td>
          <td>{count(row.sessions)}</td>
          <td class="muted-cell">{row.tool_mix}</td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty-cell">no project rows</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ProjectToolTable(rows: ProjectToolMetric[])}
  <table class="data-table project-tool-table">
    <thead><tr><th></th><th>project</th><th>tool</th><th>cost</th><th>calls</th><th>sess</th><th>avg/s</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.project} ${row.tool} rank`} /></td>
          <td>{row.project}</td>
          <td>{row.tool}</td>
          <td class="money">{row.cost}</td>
          <td>{count(row.calls)}</td>
          <td>{count(row.sessions)}</td>
          <td class="money">{row.avg_per_session}</td>
        </tr>
      {:else}
        <tr><td colspan="7" class="empty-cell">no project/tool rows</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet SessionTable(rows: SessionMetric[])}
  <table class="data-table session-table">
    <thead><tr><th></th><th>date</th><th>project</th><th>cost</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.project} session rank`} /></td>
          <td>{row.date}</td>
          <td>{row.project}</td>
          <td class="money">{row.cost}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="empty-cell">no sessions</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ModelTable(rows: ModelMetric[])}
  <table class="data-table model-table">
    <thead><tr><th></th><th>model</th><th>cost</th><th>cache</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} rank`} /></td>
          <td>{row.name}</td>
          <td class="money">{row.cost}</td>
          <td>{row.cache}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="empty-cell">no models</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet CountTable(rows: CountMetric[])}
  <table class="data-table count-table">
    <thead><tr><th></th><th>name</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows as row}
        <tr>
          <td><RankBar value={row.value} ariaLabel={`${row.name} rank`} /></td>
          <td>{row.name}</td>
          <td>{count(row.calls)}</td>
        </tr>
      {:else}
        <tr><td colspan="3" class="empty-cell">no rows</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet SessionDetailPanel(session: SessionDetailView | null)}
  <section class="session-page">
    <div class="session-head">
      <button type="button" onclick={() => commit(() => api.closeSession())}><ArrowLeft size={15} /> Deep Dive</button>
      {#if session}
        <div>
          <strong>{session.project}</strong>
          <span>{session.tool} · {session.date_range}</span>
        </div>
      {/if}
    </div>
    {#if session}
      <section class="kpis session-kpis">
        <div><span>cost</span><strong>{session.total_cost}</strong><small>{session.total_calls} calls</small></div>
        <div><span>input</span><strong>{session.total_input}</strong><small>tokens</small></div>
        <div><span>output</span><strong>{session.total_output}</strong><small>tokens</small></div>
        <div><span>cache read</span><strong>{session.total_cache_read}</strong><small>tokens</small></div>
      </section>
      <div class="session-panel-area">
        {#if session.note}<div class="status-line">{session.note}</div>{/if}
        <Panel title="Calls" tone="red">
          <table>
            <thead><tr><th>time</th><th>model</th><th>cost</th><th>in</th><th>out</th><th>cache</th><th>tools</th><th>prompt</th></tr></thead>
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
        <div class="empty-state">no session selected</div>
      </div>
    {/if}
  </section>
{/snippet}
