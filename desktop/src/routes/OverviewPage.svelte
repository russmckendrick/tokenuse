<script lang="ts">
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import GaugeBar from '../components/GaugeBar.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, LimitMetric } from '../types';

  export let snapshot: DesktopSnapshot;

  type UtilisationRow = { tool: string; limit: LimitMetric };

  function utilisationRows(snapshotValue: DesktopSnapshot): UtilisationRow[] {
    const rows: UtilisationRow[] = [];
    for (const section of snapshotValue.usage.sections) {
      for (const limit of section.limits) {
        if (!limit.stale) {
          rows.push({ tool: section.tool, limit });
        }
      }
    }
    return rows;
  }

  $: summary = snapshot.dashboard.summary;
  $: callsSub = `${summary.input} ${snapshot.copy.metrics.in}`;
  $: cacheSub = `${summary.cached} ${snapshot.copy.metrics.cached}`;
  $: outSub = `${summary.output} ${snapshot.copy.metrics.out}`;
  $: utilisation = utilisationRows(snapshot);
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <section class="kpis hero-kpis">
    <div>
      <span>{snapshot.copy.metrics.cost}</span>
      <strong class="hero-value" use:countUp={summary.cost}>{summary.cost}</strong>
      <small>{snapshot.currency}</small>
    </div>
    <div>
      <span>{snapshot.copy.metrics.calls}</span>
      <strong use:countUp={summary.calls}>{summary.calls}</strong>
      <small use:countUp={callsSub}>{callsSub}</small>
    </div>
    <div>
      <span>{snapshot.copy.metrics.sessions}</span>
      <strong use:countUp={summary.sessions}>{summary.sessions}</strong>
      <small>{snapshot.copy.metrics.active_set}</small>
    </div>
    <div>
      <span>{snapshot.copy.metrics.cache_hit}</span>
      <strong use:countUp={summary.cache_hit}>{summary.cache_hit}</strong>
      <small use:countUp={cacheSub}>{cacheSub}</small>
    </div>
    <div>
      <span>{snapshot.copy.metrics.in} / {snapshot.copy.metrics.out}</span>
      <strong use:countUp={summary.input}>{summary.input}</strong>
      <small use:countUp={outSub}>{outSub}</small>
    </div>
  </section>

  {#if utilisation.length}
    <Panel title={snapshot.copy.desktop.utilisation} tone="green">
      <div class="utilisation-strip">
        {#each utilisation as row}
          <div class="utilisation-card">
            <div class="utilisation-head">
              <strong>{row.tool}</strong>
              <span>{row.limit.scope} {row.limit.window}</span>
            </div>
            <GaugeBar used={row.limit.used} ariaLabel={`${row.tool} ${row.limit.scope}`} />
            <div class="utilisation-foot">
              <span class="mono">{row.limit.left}</span>
              <span class="muted-cell">{row.limit.reset}</span>
            </div>
          </div>
        {/each}
      </div>
    </Panel>
  {/if}

  <Panel title={snapshot.copy.panels.activity_pulse} tone="cyan">
    <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
  </Panel>

  <section class="duo-grid">
    <Panel title={snapshot.copy.desktop.top_projects} tone="yellow">
      <ProjectTable rows={snapshot.dashboard.projects} copy={snapshot.copy} />
    </Panel>
    <Panel title={snapshot.copy.desktop.top_models} tone="magenta">
      <ModelTable rows={snapshot.dashboard.models} copy={snapshot.copy} />
    </Panel>
  </section>
</section>
