<script lang="ts">
  import { api } from '../api';
  import UsageConsole from '../components/UsageConsole.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import SessionTable from '../components/tables/SessionTable.svelte';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { RouteToolId } from '../lib/router.svelte';
  import type { DesktopSnapshot, ToolPageData } from '../types';

  export let snapshot: DesktopSnapshot;
  export let tool: RouteToolId | undefined;
  export let usageTone: (tool: string, index: number) => string;

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
  <section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
    {#each snapshot.usage.sections as consoleSection, index}
      <UsageConsole section={consoleSection} tone={usageTone(consoleSection.tool, index)} copy={snapshot.copy} />
    {/each}
  </section>
{:else if toolPage}
  <section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
    <section class="kpis hero-kpis tool-hero">
      <div class="tool-hero-mark">
        <ProviderIcon id={tool} kind="tool" size={28} />
      </div>
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

    {#if section}
      <UsageConsole {section} tone={usageTone(section.tool, 0)} copy={snapshot.copy} />
    {/if}

    <section class="duo-grid">
      <Panel title={snapshot.copy.desktop.top_projects} tone="green">
        <ProjectTable rows={toolPage.dashboard.projects} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.desktop.top_models} tone="magenta">
        <ModelTable rows={toolPage.dashboard.models} copy={snapshot.copy} />
      </Panel>
    </section>

    <Panel title={snapshot.copy.panels.top_sessions} tone="red">
      <SessionTable rows={toolPage.dashboard.sessions} copy={snapshot.copy} />
    </Panel>
  </section>
{/if}
