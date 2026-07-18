<script lang="ts">
  import { api } from '../api';
  import GaugeBar from '../components/GaugeBar.svelte';
  import RankBar from '../components/RankBar.svelte';
  import UsageActivityChart from '../components/UsageActivityChart.svelte';
  import UsageConsole from '../components/UsageConsole.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import SessionTable from '../components/tables/SessionTable.svelte';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { Route, RouteToolId } from '../lib/router.svelte';
  import type { DesktopSnapshot, PlanValueMetric, ToolLimitSection, ToolPageData } from '../types';

  export let snapshot: DesktopSnapshot;
  export let tool: RouteToolId | undefined;
  export let usageTone: (tool: string, index: number) => string;
  export let navigate: (route: Route) => void;
  export let openSession: (key: string) => void;
  export let openProject: (name: string) => void;
  export let openModel: (id: string, name: string) => void;

  function routeIdFor(label: string): RouteToolId | null {
    const match = snapshot.tools.find((option) => option.value !== 'all' && option.label === label);
    return match ? (match.value as RouteToolId) : null;
  }

  // Busiest first, matching the sidebar tool rows' dynamic order.
  $: orderedSections = snapshot.usage.sections
    .slice()
    .sort((left, right) => right.usage.calls - left.usage.calls || left.tool.localeCompare(right.tool));

  function hasActivity(section: ToolLimitSection): boolean {
    return section.usage.calls > 0 || section.usage.buckets.some((bucket) => bucket > 0);
  }

  function cardLimits(section: ToolLimitSection) {
    return section.limits.filter((limit) => !limit.stale).slice(0, 3);
  }

  function openLabel(label: string) {
    return snapshot.copy.usage.open_tool.replace('{tool}', label);
  }

  function count(value: number) {
    return value.toLocaleString();
  }

  function planValueLine(planValue: PlanValueMetric) {
    return snapshot.copy.usage.plan_value_line
      .split('{month_cost}')
      .join(planValue.month_cost)
      .split('{price}')
      .join(planValue.price)
      .split('{multiple}')
      .join(planValue.multiple);
  }

  let toolPage: ToolPageData | null = null;
  let toolPageKey = '';

  $: toolLabel = tool ? snapshot.tools.find((t) => t.value === tool)?.label ?? tool : '';
  $: {
    const key = tool
      ? [tool, snapshot.period, snapshot.sort, snapshot.data_generation, snapshot.currency].join('|')
      : '';
    if (key !== toolPageKey) {
      toolPageKey = key;
      toolPage = null;
      if (tool) void loadToolPage(tool);
    }
  }

  async function loadToolPage(target: RouteToolId) {
    try {
      const page = await api.getToolPage(target);
      if (target === tool) toolPage = page;
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  $: section = toolPage?.usage.sections.find((s) => s.tool === toolLabel) ?? toolPage?.usage.sections[0];
</script>

{#if !tool}
  <section class="page-flow" use:staggeredReveal={{ selector: '.tool-card', y: 5, stagger: 0.03 }}>
    <section class="tool-fleet">
      {#each orderedSections as consoleSection, index (consoleSection.tool)}
        {@const target = routeIdFor(consoleSection.tool)}
        {@const active = hasActivity(consoleSection)}
        {@const limits = cardLimits(consoleSection)}
        {@const hero = index === 0}
        <button
          type="button"
          class="tool-card"
          class:hero
          title={openLabel(consoleSection.tool)}
          disabled={!target}
          onclick={() => target && navigate({ page: 'tools', tool: target })}
        >
          <span class="tool-card-head">
            {#if target}
              <ProviderIcon id={target} kind="tool" size={20} />
            {/if}
            <strong>{consoleSection.tool}</strong>
            <span class="tool-card-cost mono" use:countUp={consoleSection.usage.cost}>{consoleSection.usage.cost}</span>
          </span>

          <span class="tool-card-top">
            <span class="tool-card-pulse">
              <UsageActivityChart
                buckets={consoleSection.usage.buckets}
                {active}
                tone={usageTone(consoleSection.tool, index)}
                ariaLabel={`${consoleSection.tool} ${snapshot.copy.tray.hours_24} ${snapshot.copy.timeline.pulse}`}
              />
            </span>

            <span class="tool-card-stats">
              <span><small>{snapshot.copy.metrics.calls}</small><strong use:countUp={count(consoleSection.usage.calls)}>{count(consoleSection.usage.calls)}</strong></span>
              <span><small>{snapshot.copy.metrics.tokens}</small><strong use:countUp={consoleSection.usage.tokens}>{consoleSection.usage.tokens}</strong></span>
              <span><small>{snapshot.copy.usage.seen}</small><strong>{consoleSection.usage.last_seen}</strong></span>
            </span>
          </span>

          {#if limits.length}
            <span class="tool-card-limits">
              {#each limits as limit}
                <span class="tool-card-limit">
                  <span class="tool-card-limit-name">{limit.scope} {limit.window}</span>
                  <GaugeBar
                    used={limit.used}
                    ariaLabel={`${consoleSection.tool} ${limit.scope} ${limit.window}`}
                    usedSuffix={snapshot.copy.usage.used_suffix}
                  />
                  <span class="tool-card-limit-left mono">{limit.left}</span>
                  <span class="tool-card-limit-reset">{limit.reset}</span>
                  {#if hero}
                    <span class="tool-card-limit-plan mono">{limit.plan === '-' ? '' : limit.plan}</span>
                  {/if}
                </span>
              {/each}
            </span>
          {/if}

          {#if hero && consoleSection.models.length}
            <span class="tool-card-models">
              <small>{snapshot.copy.usage.models}</small>
              {#each consoleSection.models as model}
                <span class="tool-card-model">
                  <span class="tool-card-model-name">{model.name}</span>
                  <RankBar
                    value={model.value}
                    ariaLabel={`${model.name}: ${count(model.calls)} ${snapshot.copy.metrics.calls}`}
                    compact
                  />
                  <span class="mono">{count(model.calls)}</span>
                  <span class="mono muted-cell">{model.tokens}</span>
                  <span class="mono money">{model.cost}</span>
                </span>
              {/each}
            </span>
          {/if}

          {#if consoleSection.plan_value}
            <span class="tool-card-plan">
              <small>{snapshot.copy.usage.plan_value}</small>
              <span class="mono">{planValueLine(consoleSection.plan_value)}</span>
            </span>
          {/if}

          {#if !active && !limits.length}
            <span class="tool-card-idle">{snapshot.copy.usage.idle}</span>
          {/if}
        </button>
      {/each}
    </section>
  </section>
{:else if toolPage}
  <section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
    <section class="tool-hero">
      <div class="tool-hero-mark">
        <ProviderIcon id={tool} kind="tool" size={38} />
      </div>
      <section class="kpis hero-kpis tool-kpis">
        <div>
          <span>{snapshot.copy.metrics.cost}</span>
          <strong class="hero-value" use:countUp={toolPage.dashboard.summary.cost}>{toolPage.dashboard.summary.cost}</strong>
          <small>{snapshot.currency}</small>
        </div>
        <div>
          <span>{snapshot.copy.metrics.calls}</span>
          <strong use:countUp={toolPage.dashboard.summary.calls}>{toolPage.dashboard.summary.calls}</strong>
          <small>{snapshot.copy.metrics.in} {toolPage.dashboard.summary.input}</small>
        </div>
        <div>
          <span>{snapshot.copy.metrics.sessions}</span>
          <strong use:countUp={toolPage.dashboard.summary.sessions}>{toolPage.dashboard.summary.sessions}</strong>
          <small>{snapshot.copy.metrics.active_set}</small>
        </div>
        <div>
          <span>{snapshot.copy.metrics.cache_hit}</span>
          <strong use:countUp={toolPage.dashboard.summary.cache_hit}>{toolPage.dashboard.summary.cache_hit}</strong>
          <small>{snapshot.copy.metrics.cached} {toolPage.dashboard.summary.cached}</small>
        </div>
      </section>
    </section>

    {#if section}
      <UsageConsole {section} tone={usageTone(section.tool, 0)} copy={snapshot.copy} />
    {/if}

    <section class="duo-grid">
      <Panel title={snapshot.copy.desktop.top_projects} tone="green" scrollable>
        <ProjectTable rows={toolPage.dashboard.projects} copy={snapshot.copy} {openProject} />
      </Panel>
      <Panel title={snapshot.copy.desktop.top_models} tone="magenta" scrollable>
        <ModelTable rows={toolPage.dashboard.models} copy={snapshot.copy} {openModel} />
      </Panel>
    </section>

    <Panel title={snapshot.copy.panels.top_sessions} tone="red" scrollable>
      <SessionTable rows={toolPage.dashboard.sessions} copy={snapshot.copy} {openSession} />
    </Panel>
  </section>
{/if}

<style>
  .tool-fleet {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-xl);
    align-items: stretch;
  }

  .tool-card.hero {
    grid-column: 1 / -1;
  }

  .tool-card-top {
    display: contents;
  }

  .tool-card.hero .tool-card-top {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(280px, 0.9fr);
    gap: var(--space-lg);
    align-items: stretch;
  }

  .tool-card.hero .tool-card-stats {
    align-content: center;
  }

  .tool-card {
    min-width: 0;
    min-height: 0;
    padding: var(--space-lg) var(--space-xl);
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    gap: var(--space-lg);
    text-align: left;
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-md);
  }

  .tool-card:hover:not(:disabled) {
    border-color: var(--color-primary);
    color: var(--color-on-surface);
  }

  .tool-card-head {
    display: flex;
    align-items: center;
    gap: var(--space-md);
  }

  .tool-card-head strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-ui);
    font-size: 13px;
    font-weight: 600;
  }

  .tool-card-cost {
    margin-left: auto;
    color: var(--color-warning);
    font-size: var(--text-display-lg);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .tool-card-pulse {
    display: block;
    min-height: 56px;
  }

  .tool-card-stats {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-md);
  }

  .tool-card-stats > span {
    min-width: 0;
    border: 1px solid var(--color-border-row);
    padding: 5px 7px;
    display: grid;
    gap: 1px;
  }

  .tool-card-stats small,
  .tool-card-plan small {
    color: var(--color-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.06em;
  }

  .tool-card-stats strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .tool-card-limits {
    display: grid;
    gap: var(--space-sm);
  }

  .tool-card-limit {
    min-height: 24px;
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(84px, 0.9fr) minmax(58px, auto) minmax(76px, 0.7fr);
    gap: var(--space-md);
    align-items: center;
  }

  .tool-card.hero .tool-card-limit {
    grid-template-columns: minmax(0, 1.3fr) minmax(110px, 1fr) minmax(64px, auto) minmax(90px, 0.8fr) minmax(110px, 0.9fr);
  }

  .tool-card-limit-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-muted);
    font-size: 12px;
  }

  .tool-card-limit-left {
    text-align: right;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .tool-card-limit-reset,
  .tool-card-limit-plan {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
    color: var(--color-muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .tool-card-limit-plan {
    color: var(--color-warning);
  }

  .tool-card-models {
    display: grid;
    gap: var(--space-sm);
    border-top: 1px solid var(--color-border-row);
    padding-top: var(--space-md);
  }

  .tool-card-models > small {
    color: var(--color-muted);
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.06em;
  }

  .tool-card-model {
    min-height: 24px;
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(110px, 1fr) minmax(64px, auto) minmax(90px, 0.8fr) minmax(110px, 0.9fr);
    gap: var(--space-md);
    align-items: center;
    font-size: 12px;
  }

  .tool-card-model-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tool-card-model .mono {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .tool-card-plan {
    display: flex;
    align-items: baseline;
    gap: var(--space-md);
    border-top: 1px solid var(--color-border-row);
    padding-top: var(--space-md);
  }

  .tool-card-plan .mono {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .tool-card-idle {
    color: var(--color-muted);
  }

  @media (max-width: 980px) {
    .tool-fleet {
      grid-template-columns: minmax(0, 1fr);
    }

    .tool-card.hero .tool-card-top {
      display: contents;
    }

    .tool-card-limit,
    .tool-card.hero .tool-card-limit {
      grid-template-columns: minmax(0, 1.3fr) minmax(84px, 0.9fr) minmax(58px, auto);
    }

    .tool-card-limit-reset,
    .tool-card-limit-plan {
      display: none;
    }

    .tool-card-model {
      grid-template-columns: minmax(0, 1.3fr) minmax(64px, auto) minmax(90px, 0.9fr);
    }

    .tool-card-model :global(.rank-bar),
    .tool-card-model .muted-cell {
      display: none;
    }
  }
</style>
