<script lang="ts">
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import CountTable from '../components/tables/CountTable.svelte';
  import KpiStrip from '../components/tables/KpiStrip.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectToolTable from '../components/tables/ProjectToolTable.svelte';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;
</script>

<section class="page overview-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <KpiStrip summary={snapshot.dashboard.summary} currency={snapshot.currency} copy={snapshot.copy} />
  <Panel title={snapshot.copy.panels.activity_pulse} tone="cyan">
    <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
  </Panel>
  <section class="grid overview-grid">
    <div class="overview-primary">
      <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow">
        <ProjectToolTable rows={snapshot.dashboard.project_tools} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="overview-side-stack">
      <Panel title={snapshot.copy.panels.by_model} tone="magenta">
        <ModelTable rows={snapshot.dashboard.models} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.panels.shell_commands} tone="orange">
        <CountTable rows={snapshot.dashboard.commands} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta">
        <CountTable rows={snapshot.dashboard.mcp_servers} copy={snapshot.copy} />
      </Panel>
    </div>
  </section>
</section>
