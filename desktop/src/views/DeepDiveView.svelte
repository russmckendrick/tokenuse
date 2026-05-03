<script lang="ts">
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import CountTable from '../components/tables/CountTable.svelte';
  import ModelTable from '../components/tables/ModelTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import ProjectToolTable from '../components/tables/ProjectToolTable.svelte';
  import SessionTable from '../components/tables/SessionTable.svelte';
  import { reveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openSessionPicker: () => void;
</script>

<section class="page deep-page" use:reveal={{ y: 5 }}>
  <section class="grid deep-grid">
    <div class="deep-trend">
      <Panel title={snapshot.copy.panels.activity_trend} tone="blue">
        <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-projects">
      <Panel title={snapshot.copy.panels.by_project} tone="green">
        <ProjectTable rows={snapshot.dashboard.projects} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-span">
      <Panel title={snapshot.copy.panels.top_sessions} tone="red">
        <button class="panel-command" type="button" onclick={openSessionPicker}>{snapshot.copy.actions.open_session_picker}</button>
        <SessionTable rows={snapshot.dashboard.sessions} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-project-tools">
      <Panel title={snapshot.copy.panels.project_spend_by_tool} tone="yellow">
        <ProjectToolTable rows={snapshot.dashboard.project_tools} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-side-stack">
      <Panel title={snapshot.copy.panels.model_efficiency} tone="magenta">
        <ModelTable rows={snapshot.dashboard.models} copy={snapshot.copy} />
      </Panel>
      <Panel title={snapshot.copy.panels.core_tools} tone="cyan">
        <CountTable rows={snapshot.dashboard.tools} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-shell">
      <Panel title={snapshot.copy.panels.shell_commands} tone="orange">
        <CountTable rows={snapshot.dashboard.commands} copy={snapshot.copy} />
      </Panel>
    </div>
    <div class="deep-mcp">
      <Panel title={snapshot.copy.panels.mcp_servers} tone="magenta">
        <CountTable rows={snapshot.dashboard.mcp_servers} copy={snapshot.copy} />
      </Panel>
    </div>
  </section>
</section>
