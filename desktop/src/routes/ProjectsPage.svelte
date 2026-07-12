<script lang="ts">
  import { api } from '../api';
  import RankBar from '../components/RankBar.svelte';
  import { count } from '../format';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import SessionView from '../views/SessionView.svelte';
  import type { DesktopSnapshot, SessionDetail, SessionDetailView, SessionOption } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openCallDetail: (call: SessionDetail) => void;
  export let handleCallRowKey: (event: KeyboardEvent, call: SessionDetail) => void;

  let selectedProject: string | null = null;
  let sessionDetail: SessionDetailView | null = null;
  let loadingSessionKey: string | null = null;

  $: projectRows = snapshot.dashboard.projects;
  $: sessions = filteredSessions(snapshot.sessions, selectedProject);

  function filteredSessions(all: SessionOption[], project: string | null): SessionOption[] {
    if (!project) return all;
    return all.filter((session) => session.project === project);
  }

  function selectProject(name: string) {
    selectedProject = selectedProject === name ? null : name;
    sessionDetail = null;
  }

  async function selectSession(session: SessionOption) {
    loadingSessionKey = session.key;
    try {
      sessionDetail = await api.getSessionDetail(session.key);
    } catch {
      sessionDetail = null;
    } finally {
      loadingSessionKey = null;
    }
  }

  function handleRowKey(event: KeyboardEvent, action: () => void) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      action();
    }
  }
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
  <section class="duo-grid">
    <Panel title={snapshot.copy.panels.by_project} tone="green" scrollable>
      <table class="data-table project-table">
        <thead>
          <tr>
            <th></th>
            <th>{snapshot.copy.tables.project}</th>
            <th>{snapshot.copy.tables.cost}</th>
            <th>{snapshot.copy.tables.avg_per_session}</th>
            <th>{snapshot.copy.tables.sess}</th>
            <th>{snapshot.copy.tables.tools}</th>
          </tr>
        </thead>
        <tbody>
          {#each projectRows as row}
            <tr
              class="click-row"
              class:selected-row={selectedProject === row.name}
              tabindex="0"
              onclick={() => selectProject(row.name)}
              onkeydown={(event) => handleRowKey(event, () => selectProject(row.name))}
            >
              <td><RankBar value={row.value} ariaLabel={`${row.name} ${snapshot.copy.desktop.rank}`} /></td>
              <td>{row.name}</td>
              <td class="money">{row.cost}</td>
              <td class="money">{row.avg_per_session}</td>
              <td>{count(row.sessions)}</td>
              <td class="muted-cell">{row.tool_mix}</td>
            </tr>
          {:else}
            <tr><td colspan="6" class="empty-cell">{snapshot.copy.empty.no_project_rows}</td></tr>
          {/each}
        </tbody>
      </table>
    </Panel>

    <Panel
      title={selectedProject
        ? `${snapshot.copy.desktop.sessions_for} · ${selectedProject}`
        : snapshot.copy.panels.top_sessions}
      tone="red"
      scrollable
    >
      <table class="data-table session-table">
        <thead>
          <tr>
            <th>{snapshot.copy.tables.date}</th>
            <th>{snapshot.copy.tables.project}</th>
            <th>{snapshot.copy.tables.tool}</th>
            <th>{snapshot.copy.tables.cost}</th>
            <th>{snapshot.copy.tables.calls}</th>
          </tr>
        </thead>
        <tbody>
          {#each sessions as session}
            <tr
              class="click-row"
              class:selected-row={sessionDetail?.key === session.key}
              class:loading-row={loadingSessionKey === session.key}
              tabindex="0"
              onclick={() => void selectSession(session)}
              onkeydown={(event) => handleRowKey(event, () => void selectSession(session))}
            >
              <td>{session.date}</td>
              <td>{session.project}</td>
              <td class="muted-cell">{session.tool}</td>
              <td class="money">{session.cost}</td>
              <td>{count(session.calls)}</td>
            </tr>
          {:else}
            <tr><td colspan="5" class="empty-cell">{snapshot.copy.empty.no_sessions}</td></tr>
          {/each}
        </tbody>
      </table>
    </Panel>
  </section>

  {#if sessionDetail}
    <SessionView
      {snapshot}
      session={sessionDetail}
      closeSession={() => (sessionDetail = null)}
      {openCallDetail}
      {handleCallRowKey}
    />
  {:else}
    <div class="projects-hint muted-cell">{snapshot.copy.desktop.call_detail_hint}</div>
  {/if}
</section>

<style>
  .selected-row {
    background: var(--color-neutral);
  }

  .loading-row {
    opacity: 0.6;
  }

  .projects-hint {
    font-family: var(--font-ui);
    font-size: 12px;
    padding: 0 2px;
  }
</style>
