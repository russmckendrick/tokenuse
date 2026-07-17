<script lang="ts">
  import { api } from '../api';
  import Donut from '../charts/Donut.svelte';
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import ActivityCategoryTable from '../components/tables/ActivityCategoryTable.svelte';
  import CountTable from '../components/tables/CountTable.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import { count, rankLabel, rankPercent } from '../format';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type {
    CoachProjectActivity,
    DesktopSnapshot,
    ProjectPageData,
    ProjectToolSplit,
    ShareMetric
  } from '../types';

  export let snapshot: DesktopSnapshot;
  export let project: { identity: string; label: string };
  export let openSession: (key: string) => void;

  const SESSION_PAGE_SIZE = 60;

  let data: ProjectPageData | null = null;
  let pageKey = '';
  let visibleSessions = SESSION_PAGE_SIZE;

  $: {
    const key = [
      project.identity,
      snapshot.period,
      snapshot.sort,
      snapshot.data_generation,
      snapshot.currency
    ].join('|');
    if (key !== pageKey) {
      pageKey = key;
      void loadPage();
    }
  }

  async function loadPage() {
    try {
      const next = await api.getProjectPage(project.identity);
      if (next.identity === project.identity) {
        data = next;
        visibleSessions = SESSION_PAGE_SIZE;
      }
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  $: sessions = data?.sessions ?? [];
  $: shownSessions = sessions.slice(0, visibleSessions);

  /** Sentinel row action: grow the rendered session window as it scrolls
   * into view, so long histories never render in one go. */
  function loadMoreSentinel(node: HTMLElement) {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        visibleSessions += SESSION_PAGE_SIZE;
      }
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function handleSessionRowKey(event: KeyboardEvent, key: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openSession(key);
    }
  }

  type SplitMode = 'cost' | 'calls' | 'sessions' | 'avg';
  let splitMode: SplitMode = 'cost';

  $: splitModes = [
    { id: 'cost' as SplitMode, label: snapshot.copy.metrics.cost },
    { id: 'calls' as SplitMode, label: snapshot.copy.metrics.calls },
    { id: 'sessions' as SplitMode, label: snapshot.copy.metrics.sessions },
    { id: 'avg' as SplitMode, label: snapshot.copy.tables.avg_per_session }
  ];

  function splitValue(row: ProjectToolSplit, mode: SplitMode): number {
    if (mode === 'calls') return row.calls;
    if (mode === 'sessions') return row.sessions;
    if (mode === 'avg') return row.avg_value;
    return row.cost_value;
  }

  function splitDisplay(row: ProjectToolSplit, mode: SplitMode): string {
    if (mode === 'calls') return count(row.calls);
    if (mode === 'sessions') return count(row.sessions);
    if (mode === 'avg') return row.avg_per_session;
    return row.cost;
  }

  $: splitRows = ((rows: ProjectToolSplit[], mode: SplitMode): ShareMetric[] => {
    const total = rows.reduce((sum, row) => sum + splitValue(row, mode), 0);
    return rows.map((row) => ({
      key: row.key,
      label: row.label,
      cost: splitDisplay(row, mode),
      calls: row.calls,
      share: total > 0 ? Math.round((splitValue(row, mode) / total) * 1000) : 0
    }));
  })(data?.tool_split ?? [], splitMode);

  function template(text: string, values: Record<string, string>): string {
    return Object.entries(values).reduce(
      (out, [name, value]) => out.split(`{${name}}`).join(value),
      text
    );
  }

  function patternLabel(activity: CoachProjectActivity | null): string {
    if (!activity?.days_id) return '';
    const timeline = snapshot.copy.coach.timeline;
    const days = timeline[`pattern_${activity.days_id}`] ?? activity.days_id;
    if (!activity.time_id) return days;
    const time = timeline[`pattern_${activity.time_id}`] ?? activity.time_id;
    return template(timeline.pattern_combo, { days, time });
  }
</script>

{#if data}
  <section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
    <section class="kpis hero-kpis project-kpis">
      <div>
        <span>{snapshot.copy.metrics.cost}</span>
        <strong class="hero-value" use:countUp={data.dashboard.summary.cost}>{data.dashboard.summary.cost}</strong>
        <small>{snapshot.currency}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.calls}</span>
        <strong use:countUp={data.dashboard.summary.calls}>{data.dashboard.summary.calls}</strong>
        <small>{snapshot.copy.metrics.in} {data.dashboard.summary.input}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.sessions}</span>
        <strong use:countUp={data.dashboard.summary.sessions}>{data.dashboard.summary.sessions}</strong>
        <small>{snapshot.copy.metrics.active_set}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.cache_hit}</span>
        <strong use:countUp={data.dashboard.summary.cache_hit}>{data.dashboard.summary.cache_hit}</strong>
        <small>{snapshot.copy.metrics.cached} {data.dashboard.summary.cached}</small>
      </div>
      <div>
        <span>{snapshot.copy.coach.output.total}</span>
        <strong use:countUp={data.output.total_loc}>{data.output.total_loc}</strong>
        <small>{data.output.by_language[0]?.name ?? snapshot.copy.tables.blank}</small>
      </div>
      <div>
        <span>{snapshot.copy.coach.timeline.est_hours}</span>
        <strong>{data.activity?.active_hours ?? snapshot.copy.tables.blank}</strong>
        <small>{patternLabel(data.activity)}</small>
      </div>
    </section>

    <Panel title={snapshot.copy.panels.activity_pulse} tone="blue">
      <ActivityPulse points={data.dashboard.activity_timeline} copy={snapshot.copy} />
    </Panel>

    <section class="duo-grid">
      <Panel title={snapshot.copy.metrics.sessions} tone="red" scrollable>
        <table class="data-table session-table">
          <thead>
            <tr>
              <th>{snapshot.copy.tables.date}</th>
              <th>{snapshot.copy.tables.tool}</th>
              <th>{snapshot.copy.tables.cost}</th>
              <th>{snapshot.copy.tables.calls}</th>
            </tr>
          </thead>
          <tbody>
            {#each shownSessions as session (session.key)}
              <tr
                class="rank-row click-row"
                style:--rank-fill={`${rankPercent(session.value)}%`}
                title={rankLabel(snapshot.copy.timeline.relative_rank, session.value)}
                tabindex="0"
                onclick={() => openSession(session.key)}
                onkeydown={(event) => handleSessionRowKey(event, session.key)}
              >
                <td>{session.date}</td>
                <td class="muted-cell">{session.tool}</td>
                <td class="money">{session.cost}</td>
                <td>{count(session.calls)}</td>
              </tr>
            {:else}
              <tr><td colspan="4" class="empty-cell">{snapshot.copy.empty.no_sessions}</td></tr>
            {/each}
            {#if visibleSessions < sessions.length}
              <tr use:loadMoreSentinel><td colspan="4" class="empty-cell">{count(sessions.length - visibleSessions)}+</td></tr>
            {/if}
          </tbody>
        </table>
      </Panel>

      <Panel title={snapshot.copy.coach.output.title} tone="magenta" scrollable>
        <CountTable rows={data.output.by_language} copy={snapshot.copy} />
        {#if data.activity?.hot_files.length}
          <div class="project-hot-files">
            <small>{snapshot.copy.coach.timeline.hot_files}</small>
            {#each data.activity.hot_files as file}
              <span class="mono">{file}</span>
            {/each}
          </div>
        {/if}
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow">
        {#if data.tool_split.length === 1}
          {@const only = data.tool_split[0]}
          <div class="single-tool">
            <div class="single-tool-head">
              {#if only.key}
                <ProviderIcon id={only.key} kind="tool" size={20} />
              {/if}
              <strong>{only.label}</strong>
            </div>
            <div class="single-tool-stats">
              <span><small>{snapshot.copy.metrics.cost}</small><strong class="mono money">{only.cost}</strong></span>
              <span><small>{snapshot.copy.metrics.calls}</small><strong class="mono">{count(only.calls)}</strong></span>
              <span><small>{snapshot.copy.metrics.sessions}</small><strong class="mono">{count(only.sessions)}</strong></span>
              <span><small>{snapshot.copy.tables.avg_per_session}</small><strong class="mono money">{only.avg_per_session}</strong></span>
            </div>
          </div>
        {:else}
          <div class="split-switch" role="group" aria-label={snapshot.copy.panels.project_spend_by_tool}>
            {#each splitModes as mode (mode.id)}
              <button
                type="button"
                class:active={splitMode === mode.id}
                aria-pressed={splitMode === mode.id}
                onclick={() => (splitMode = mode.id)}
              >{mode.label}</button>
            {/each}
          </div>
          <Donut
            rows={splitRows}
            colorBy="tool"
            centerLabel={splitModes.find((mode) => mode.id === splitMode)?.label ?? ''}
            ariaLabel={snapshot.copy.panels.project_spend_by_tool}
            emptyLabel={snapshot.copy.empty.no_data}
          />
        {/if}
      </Panel>
      <Panel title={snapshot.copy.desktop.top_models} tone="magenta" scrollable>
        <ModelTable rows={data.dashboard.models} copy={snapshot.copy} />
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.categories.heading} tone="cyan" scrollable>
        <ActivityCategoryTable rows={data.dashboard.by_activity} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.panels.core_tools} tone="cyan" scrollable>
        <CountTable rows={data.dashboard.tools} copy={snapshot.copy} />
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.panels.shell_commands} tone="orange" scrollable>
        <CountTable rows={data.dashboard.commands} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta" scrollable>
        <CountTable rows={data.dashboard.mcp_servers} copy={snapshot.copy} />
      </Panel>
    </section>

    <Panel title={snapshot.copy.desktop.project_sources} tone="green" scrollable>
      <table class="data-table project-sources-table">
        <thead>
          <tr>
            <th>{snapshot.copy.tables.tool}</th>
            <th>{snapshot.copy.tables.raw_project}</th>
            <th>{snapshot.copy.tables.calls}</th>
            <th>{snapshot.copy.tables.sess}</th>
            <th>{snapshot.copy.tables.cost}</th>
          </tr>
        </thead>
        <tbody>
          {#each data.sources as source (`${source.tool}:${source.raw_project}`)}
            <tr>
              <td>{source.tool}</td>
              <td class="mono muted-cell">{source.raw_project}</td>
              <td>{count(source.calls)}</td>
              <td>{count(source.sessions)}</td>
              <td class="money">{source.cost}</td>
            </tr>
          {:else}
            <tr><td colspan="5" class="empty-cell">{snapshot.copy.empty.no_project_rows}</td></tr>
          {/each}
        </tbody>
      </table>
    </Panel>
  </section>
{/if}

<style>
  .project-kpis {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .split-switch {
    display: flex;
    gap: 4px;
    margin-bottom: 10px;
  }

  .split-switch button {
    font-family: var(--font-mono);
    font-size: var(--text-label);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-muted);
    background: transparent;
    border: 1px solid var(--color-border-soft);
    padding: 3px 8px;
    cursor: pointer;
  }

  .split-switch button:hover {
    color: var(--color-on-surface);
  }

  .split-switch button.active {
    color: var(--color-on-surface);
    border-color: var(--color-primary);
    background: color-mix(in srgb, var(--color-primary) 12%, transparent);
  }

  /* One tool means there is no split to chart; show that tool's numbers
   * plainly instead of a meaningless 100% ring. */
  .single-tool {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 6px 2px;
  }

  .single-tool-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-body);
  }

  .single-tool-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: 10px;
  }

  .single-tool-stats span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .single-tool-stats small {
    color: var(--color-muted-2);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: var(--text-label);
  }

  .single-tool-stats strong {
    font-size: var(--text-display);
    font-weight: 600;
  }

  .project-hot-files {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 2px 2px;
    font-size: 12px;
  }

  .project-hot-files small {
    color: var(--color-muted-2);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: var(--text-label);
  }

  .project-hot-files .mono {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
