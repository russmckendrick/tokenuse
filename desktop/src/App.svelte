<script lang="ts">
  import { onMount } from 'svelte';
  import { confirm, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { ArrowLeft, Database, Download, FolderOpen, RefreshCw, Search, X } from 'lucide-svelte';
  import { api } from './api';
  import Panel from './Panel.svelte';
  import type {
    ConfigRow,
    CountMetric,
    DailyMetric,
    DesktopSnapshot,
    ExportFormatId,
    ModelMetric,
    PageId,
    ProjectMetric,
    ProjectOption,
    ProjectToolMetric,
    RecentModelMetric,
    SessionDetailView,
    SessionMetric,
    SessionOption,
    Summary,
    ToolId,
    ToolLimitSection
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

  function handleKey(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.tagName === 'INPUT' || target?.tagName === 'SELECT') {
      if (event.key === 'Escape') closeModal();
      return;
    }
    if (!snapshot) return;

    if (modal) {
      if (event.key === 'Escape') closeModal();
      return;
    }

    if (event.key === 'Escape' && snapshot.page === 'session') {
      event.preventDefault();
      void commit(() => api.closeSession());
      return;
    }

    const periodKeys = ['today', 'week', 'thirty-days', 'month', 'all-time'] as const;
    if (/^[1-5]$/.test(event.key)) {
      event.preventDefault();
      const period = periodKeys[Number(event.key) - 1];
      void commit(() => api.setPeriod(period));
      return;
    }

    switch (event.key.toLowerCase()) {
      case 'o':
        event.preventDefault();
        void commit(() => api.setPage('overview'));
        break;
      case 'd':
        event.preventDefault();
        void commit(() => api.setPage('deep-dive'));
        break;
      case 'u':
        event.preventDefault();
        void commit(() => api.setPage('usage'));
        break;
      case 'c':
        event.preventDefault();
        void commit(() => api.setPage('config'));
        break;
      case 'p':
        event.preventDefault();
        openModal('project');
        break;
      case 's':
        event.preventDefault();
        openModal('session');
        break;
      case 'e':
        event.preventDefault();
        openModal('export');
        break;
      case 'r':
        event.preventDefault();
        void commit(() => api.refreshArchive());
        break;
      case 't':
        event.preventDefault();
        cycleTool();
        break;
    }
  }

  function cycleTool() {
    if (!snapshot) return;
    const idx = snapshot.tools.findIndex((tool) => tool.value === snapshot?.tool);
    const next = snapshot.tools[(idx + 1) % snapshot.tools.length];
    void commit(() => api.setTool(next.value));
  }

  function setToolFromEvent(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as ToolId;
    void commit(() => api.setTool(value));
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

  function bar(value: number) {
    return Math.max(0, Math.min(100, value));
  }

  function count(value: number) {
    return value.toLocaleString();
  }

  function activePage() {
    return snapshot?.page ?? 'overview';
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
        <span class="brand-mark">tokenuse</span>
        <span class="version">v{snapshot.version}</span>
        <span class:live={snapshot.source === 'live'} class="source">{snapshot.source}</span>
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
        {@render Kpis(snapshot.dashboard.summary, snapshot.currency)}
        <section class="grid overview-grid">
          <Panel title="Daily Activity" tone="blue">
            {@render DailyTable(snapshot.dashboard.daily, 8)}
          </Panel>
          <Panel title="By Model" tone="magenta">
            {@render ModelTable(snapshot.dashboard.models, 8)}
          </Panel>
          <Panel title="Project Spend by Tool" tone="yellow">
            {@render ProjectToolTable(snapshot.dashboard.project_tools, 10)}
          </Panel>
          <div class="stack">
            <Panel title="Shell Commands" tone="orange">
              {@render CountTable(snapshot.dashboard.commands, 6)}
            </Panel>
            <Panel title="MCP Servers" tone="magenta">
              {@render CountTable(snapshot.dashboard.mcp_servers, 6)}
            </Panel>
          </div>
        </section>
      {:else if activePage() === 'deep-dive'}
        <section class="grid deep-grid">
          <Panel title="Daily Activity" tone="blue">
            {@render DailyTable(snapshot.dashboard.daily, 10)}
          </Panel>
          <Panel title="By Project" tone="green">
            {@render ProjectTable(snapshot.dashboard.projects, 10)}
          </Panel>
          <Panel title="Top Sessions" tone="red">
            <button class="panel-command" type="button" onclick={() => openModal('session')}>Open Session Picker</button>
            {@render SessionTable(snapshot.dashboard.sessions, 12)}
          </Panel>
          <Panel title="Project Spend by Tool" tone="yellow">
            {@render ProjectToolTable(snapshot.dashboard.project_tools, 14)}
          </Panel>
          <div class="stack">
            <Panel title="By Model" tone="magenta">
              {@render ModelTable(snapshot.dashboard.models, 7)}
            </Panel>
            <Panel title="Core Tools" tone="cyan">
              {@render CountTable(snapshot.dashboard.tools, 7)}
            </Panel>
          </div>
          <Panel title="Shell Commands" tone="orange">
            {@render CountTable(snapshot.dashboard.commands, 10)}
          </Panel>
          <Panel title="MCP Servers" tone="magenta">
            {@render CountTable(snapshot.dashboard.mcp_servers, 10)}
          </Panel>
        </section>
      {:else if activePage() === 'usage'}
        <section class="usage-grid">
          {#each snapshot.usage.sections as section}
            {@render UsagePanel(section)}
          {/each}
        </section>
      {:else if activePage() === 'config'}
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
      {:else if activePage() === 'session'}
        {@render SessionDetailPanel(snapshot.session)}
      {/if}
    </main>

    <footer>
      <span><b>1-5</b> period</span>
      <span><b>t</b> tool</span>
      <span><b>p</b> project</span>
      <span><b>s</b> session</span>
      <span><b>e</b> export</span>
      <span><b>r</b> refresh</span>
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
{:else}
  <div class="loading">tokenuse</div>
{/if}

{#snippet heat(value: number)}
  <span class="heat" aria-hidden="true">
    {#each Array.from({ length: 10 }) as _, index}
      <span class:filled={index < Math.ceil((bar(value) / 100) * 10)}></span>
    {/each}
  </span>
{/snippet}

{#snippet sparkline(values: number[])}
  <span class="sparkline" aria-hidden="true">
    {#each values as value}
      <span style={`height: ${Math.max(8, bar(value))}%`}></span>
    {/each}
  </span>
{/snippet}

{#snippet Kpis(summary: Summary, currency: string)}
  <section class="kpis">
    <div><span>cost</span><strong>{summary.cost}</strong><small>{currency}</small></div>
    <div><span>calls</span><strong>{summary.calls}</strong><small>{summary.input} in</small></div>
    <div><span>sessions</span><strong>{summary.sessions}</strong><small>active set</small></div>
    <div><span>cache hit</span><strong>{summary.cache_hit}</strong><small>{summary.cached} cached</small></div>
    <div><span>in / out</span><strong>{summary.input}</strong><small>{summary.output} out</small></div>
  </section>
{/snippet}

{#snippet DailyTable(rows: DailyMetric[], limit: number)}
  <table>
    <thead><tr><th>date</th><th></th><th>cost</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr><td>{row.day}</td><td>{@render heat(row.value)}</td><td class="money">{row.cost}</td><td>{count(row.calls)}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ProjectTable(rows: ProjectMetric[], limit: number)}
  <table>
    <thead><tr><th></th><th>project</th><th>cost</th><th>avg/s</th><th>sess</th><th>tools</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr>
          <td>{@render heat(row.value)}</td>
          <td>{row.name}</td>
          <td class="money">{row.cost}</td>
          <td class="money">{row.avg_per_session}</td>
          <td>{count(row.sessions)}</td>
          <td class="muted-cell">{row.tool_mix}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ProjectToolTable(rows: ProjectToolMetric[], limit: number)}
  <table>
    <thead><tr><th></th><th>project</th><th>tool</th><th>cost</th><th>calls</th><th>sess</th><th>avg/s</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr>
          <td>{@render heat(row.value)}</td>
          <td>{row.project}</td>
          <td>{row.tool}</td>
          <td class="money">{row.cost}</td>
          <td>{count(row.calls)}</td>
          <td>{count(row.sessions)}</td>
          <td class="money">{row.avg_per_session}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet SessionTable(rows: SessionMetric[], limit: number)}
  <table>
    <thead><tr><th></th><th>date</th><th>project</th><th>cost</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr><td>{@render heat(row.value)}</td><td>{row.date}</td><td>{row.project}</td><td class="money">{row.cost}</td><td>{count(row.calls)}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet ModelTable(rows: ModelMetric[], limit: number)}
  <table>
    <thead><tr><th></th><th>model</th><th>cost</th><th>cache</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr><td>{@render heat(row.value)}</td><td>{row.name}</td><td class="money">{row.cost}</td><td>{row.cache}</td><td>{count(row.calls)}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet CountTable(rows: CountMetric[], limit: number)}
  <table>
    <thead><tr><th></th><th>name</th><th>calls</th></tr></thead>
    <tbody>
      {#each rows.slice(0, limit) as row}
        <tr><td>{@render heat(row.value)}</td><td>{row.name}</td><td>{count(row.calls)}</td></tr>
      {/each}
    </tbody>
  </table>
{/snippet}

{#snippet UsagePanel(section: ToolLimitSection)}
  <Panel title={`${section.tool} · 24h usage + models`} tone="cyan">
    <div class="usage-row">
      <span>usage</span>
      {@render sparkline(section.usage.buckets)}
      <strong>{count(section.usage.calls)}</strong>
      <span>{section.usage.tokens}</span>
      <span class="money">{section.usage.cost}</span>
      <span>{section.usage.last_seen}</span>
    </div>
    {#each section.limits as limit}
      <div class="usage-row limit-row">
        <span>{limit.scope}</span>
        {@render heat(limit.used)}
        <strong>{limit.left}</strong>
        <span>{limit.window}</span>
        <span>{limit.reset}</span>
        <span>{limit.plan}</span>
      </div>
    {/each}
    {#each section.models as model}
      {@render RecentModelRow(model)}
    {/each}
  </Panel>
{/snippet}

{#snippet RecentModelRow(model: RecentModelMetric)}
  <div class="usage-row">
    <span>{model.name}</span>
    {@render heat(model.value)}
    <strong>{count(model.calls)}</strong>
    <span>{model.tokens}</span>
    <span class="money">{model.cost}</span>
    <span></span>
  </div>
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
      {#if session.note}<div class="status-line">{session.note}</div>{/if}
      <Panel title="Calls" tone="red">
        <table>
          <thead><tr><th>time</th><th>model</th><th>cost</th><th>in</th><th>out</th><th>cache</th><th>tools</th><th>prompt</th></tr></thead>
          <tbody>
            {#each session.calls as call}
              <tr>
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
    {:else}
      <div class="empty-state">no session selected</div>
    {/if}
  </section>
{/snippet}
