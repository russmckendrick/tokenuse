<script lang="ts">
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import GaugeBar from '../components/GaugeBar.svelte';
  import ToolGauge from '../components/ToolGauge.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, LimitMetric } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openProject: (name: string) => void;

  type UtilisationTool = {
    id: string;
    tool: string;
    limits: LimitMetric[];
    peak: number;
  };

  function canonicalToolId(value: string): string {
    const normalized = value.toLowerCase();
    if (normalized.includes('claude')) return 'claude-code';
    if (normalized.includes('codex')) return 'codex';
    if (normalized.includes('copilot')) return 'copilot';
    if (normalized.includes('cursor')) return 'cursor';
    if (normalized.includes('gemini')) return 'gemini';
    return normalized;
  }

  function showOverviewLimit(limit: LimitMetric): boolean {
    if (limit.stale) return false;
    const tool = canonicalToolId(limit.tool);
    const scope = limit.scope.toLowerCase();
    if (tool === 'claude-code' && scope === 'extra usage') return false;
    if (tool === 'codex' && scope === 'gpt-5.3-codex-spark') return false;
    return true;
  }

  function utilisationTools(snapshotValue: DesktopSnapshot): UtilisationTool[] {
    const tools: UtilisationTool[] = [];
    for (const section of snapshotValue.usage.sections) {
      const limits = section.limits.filter(showOverviewLimit);
      if (!limits.length) continue;
      tools.push({
        id: canonicalToolId(limits[0]?.tool || section.tool),
        tool: section.tool,
        limits,
        peak: Math.max(...limits.map((limit) => limit.used))
      });
    }
    return tools;
  }

  $: summary = snapshot.dashboard.summary;
  $: callsSub = `${summary.input} ${snapshot.copy.metrics.in}`;
  $: cacheSub = `${summary.cached} ${snapshot.copy.metrics.cached}`;
  $: outSub = `${summary.output} ${snapshot.copy.metrics.out}`;
  $: utilisation = utilisationTools(snapshot);
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
        {#each utilisation as tool}
          <section class="utilisation-tool" style={`--limit-weight:${Math.max(2, tool.limits.length)}`}>
            <div class="utilisation-identity">
              <ToolGauge
                id={tool.id}
                used={tool.peak}
                ariaLabel={tool.tool}
                usedSuffix={snapshot.copy.usage.used_suffix}
              />
              <strong>{tool.tool}</strong>
            </div>
            <div class="utilisation-limits">
              {#each tool.limits as limit}
                <div class="utilisation-limit">
                  <div class="utilisation-head">
                    <strong>{limit.scope} <span>{limit.window}</span></strong>
                    <span class="mono">{limit.left}</span>
                  </div>
                  <GaugeBar
                    used={limit.used}
                    ariaLabel={`${tool.tool} ${limit.scope}`}
                    usedSuffix={snapshot.copy.usage.used_suffix}
                  />
                  <div class="utilisation-foot">
                    <span class="mono">{Math.round(limit.used)}% {snapshot.copy.usage.used_suffix}</span>
                    <span class="muted-cell">{snapshot.copy.tables.reset} {limit.reset}</span>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/each}
      </div>
    </Panel>
  {/if}

  <Panel title={snapshot.copy.panels.activity_pulse} tone="cyan">
    <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
  </Panel>

  <section class="duo-grid">
    <Panel title={snapshot.copy.desktop.top_projects} tone="yellow" scrollable>
      <ProjectTable rows={snapshot.dashboard.projects} copy={snapshot.copy} {openProject} />
    </Panel>
    <Panel title={snapshot.copy.desktop.top_models} tone="magenta" scrollable>
      <ModelTable rows={snapshot.dashboard.models} copy={snapshot.copy} />
    </Panel>
  </section>
</section>
