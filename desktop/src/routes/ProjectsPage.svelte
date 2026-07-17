<script lang="ts">
  import { api } from '../api';
  import { count, rankLabel, rankPercent } from '../format';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, ProjectIndexRow } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openProjectPage: (identity: string | null, label: string) => void;

  let rows: ProjectIndexRow[] = [];
  let loaded = false;
  let indexKey = '';

  $: {
    const key = [
      snapshot.period,
      snapshot.tool,
      snapshot.project.identity ?? '',
      snapshot.sort,
      snapshot.data_generation,
      snapshot.currency
    ].join('|');
    if (key !== indexKey) {
      indexKey = key;
      void loadIndex();
    }
  }

  async function loadIndex() {
    try {
      rows = await api.getProjectIndex();
      loaded = true;
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  function handleRowKey(event: KeyboardEvent, row: ProjectIndexRow) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openProjectPage(row.identity, row.name);
    }
  }
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
  <Panel title={snapshot.copy.panels.by_project} tone="green">
    <table class="data-table project-index-table">
      <thead>
        <tr>
          <th>{snapshot.copy.tables.project}</th>
          <th>{snapshot.copy.tables.cost}</th>
          <th>{snapshot.copy.tables.avg_per_session}</th>
          <th>{snapshot.copy.tables.sess}</th>
          <th>{snapshot.copy.tables.calls}</th>
          <th>{snapshot.copy.tables.last_active}</th>
          <th>{snapshot.copy.tables.tools}</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as row (row.identity)}
          <tr
            class="rank-row click-row"
            style:--rank-fill={`${rankPercent(row.value)}%`}
            title={rankLabel(snapshot.copy.timeline.relative_rank, row.value)}
            tabindex="0"
            onclick={() => openProjectPage(row.identity, row.name)}
            onkeydown={(event) => handleRowKey(event, row)}
          >
            <td>{row.name}<span class="sr-only">{rankLabel(snapshot.copy.timeline.relative_rank, row.value)}</span></td>
            <td class="money">{row.cost}</td>
            <td class="money">{row.avg_per_session}</td>
            <td>{count(row.sessions)}</td>
            <td>{count(row.calls)}</td>
            <td class="muted-cell">{row.last_active}</td>
            <td class="muted-cell">{row.tool_mix}</td>
          </tr>
        {:else}
          {#if loaded}
            <tr><td colspan="7" class="empty-cell">{snapshot.copy.empty.no_project_rows}</td></tr>
          {/if}
        {/each}
      </tbody>
    </table>
  </Panel>
</section>
