<script lang="ts">
  import { api } from '../api';
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import CountTable from '../components/tables/CountTable.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import ProjectToolTable from '../components/tables/ProjectToolTable.svelte';
  import SessionTable from '../components/tables/SessionTable.svelte';
  import Donut from '../charts/Donut.svelte';
  import Heatmap from '../charts/Heatmap.svelte';
  import StackedBars from '../charts/StackedBars.svelte';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { AnalyticsData, DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openSessionPicker: () => void;

  let analytics: AnalyticsData | null = null;
  let analyticsKey = '';

  $: weekdays = (snapshot.copy.export.calendar_weekdays as string[]) ?? [];
  $: {
    const key = [
      snapshot.period,
      snapshot.tool,
      snapshot.project.identity ?? '',
      snapshot.data_generation,
      snapshot.currency
    ].join('|');
    if (key !== analyticsKey) {
      analyticsKey = key;
      void loadAnalytics();
    }
  }

  async function loadAnalytics() {
    try {
      analytics = await api.getAnalytics(snapshot.period);
    } catch {
      // Keep the previous analytics render on transient errors.
    }
  }
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <Panel title={snapshot.copy.panels.activity_trend} tone="blue">
    <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
  </Panel>

  {#if analytics}
    <section class="duo-grid">
      <Panel title={snapshot.copy.desktop.daily_by_tool} tone="orange">
        <StackedBars
          rows={analytics.daily_by_tool}
          ariaLabel={snapshot.copy.desktop.daily_by_tool}
          emptyLabel={snapshot.copy.empty.no_data}
        />
      </Panel>
      <Panel title={snapshot.copy.desktop.hour_heatmap} tone="cyan">
        <Heatmap
          matrix={analytics.hour_day}
          dayLabels={weekdays}
          ariaLabel={snapshot.copy.desktop.hour_heatmap}
          emptyLabel={snapshot.copy.empty.no_data}
        />
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.desktop.by_provider} tone="magenta">
        <Donut
          rows={analytics.provider_share}
          colorBy="provider"
          centerLabel={snapshot.copy.desktop.spend_share}
          ariaLabel={snapshot.copy.desktop.by_provider}
          emptyLabel={snapshot.copy.empty.no_data}
        />
      </Panel>
      <Panel title={snapshot.copy.desktop.by_tool} tone="green">
        <Donut
          rows={analytics.tool_share}
          colorBy="tool"
          centerLabel={snapshot.copy.desktop.spend_share}
          ariaLabel={snapshot.copy.desktop.by_tool}
          emptyLabel={snapshot.copy.empty.no_data}
        />
      </Panel>
    </section>
  {/if}

  <section class="duo-grid">
    <Panel title={snapshot.copy.panels.by_project} tone="green" scrollable>
      <ProjectTable rows={snapshot.dashboard.projects} copy={snapshot.copy} />
    </Panel>
    <Panel title={snapshot.copy.panels.model_efficiency} tone="magenta" scrollable>
      <ModelTable rows={snapshot.dashboard.models} copy={snapshot.copy} />
    </Panel>
  </section>

  <Panel title={snapshot.copy.panels.top_sessions} tone="red" scrollable>
    <button class="panel-command" type="button" onclick={openSessionPicker}>{snapshot.copy.actions.open_session_picker}</button>
    <SessionTable rows={snapshot.dashboard.sessions} copy={snapshot.copy} />
  </Panel>

  <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow" scrollable>
    <ProjectToolTable rows={snapshot.dashboard.project_tools} copy={snapshot.copy} />
  </Panel>

  <section class="trio-grid">
    <Panel title={snapshot.copy.panels.core_tools} tone="cyan" scrollable>
      <CountTable rows={snapshot.dashboard.tools} copy={snapshot.copy} />
    </Panel>
    <Panel title={snapshot.copy.panels.shell_commands} tone="orange" scrollable>
      <CountTable rows={snapshot.dashboard.commands} copy={snapshot.copy} />
    </Panel>
    <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta" scrollable>
      <CountTable rows={snapshot.dashboard.mcp_servers} copy={snapshot.copy} />
    </Panel>
  </section>
</section>
