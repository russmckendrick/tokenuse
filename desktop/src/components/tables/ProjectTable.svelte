<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, ProjectMetric } from '../../types';

  export let rows: ProjectMetric[] = [];
  export let copy: CopyDeck;
  export let openProject: ((name: string) => void) | null = null;

  function handleRowKey(event: KeyboardEvent, name: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openProject?.(name);
    }
  }
</script>

<table class="data-table project-table">
  <thead><tr><th>{copy.tables.project}</th><th>{copy.tables.cost}</th><th>{copy.tables.avg_per_session}</th><th>{copy.tables.sess}</th><th>{copy.tables.tools}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr
        class="rank-row"
        class:click-row={Boolean(openProject)}
        style:--rank-fill={`${rankPercent(row.value)}%`}
        title={rankLabel(copy.timeline.relative_rank, row.value)}
        tabindex={openProject ? 0 : undefined}
        onclick={openProject ? () => openProject?.(row.name) : undefined}
        onkeydown={openProject ? (event) => handleRowKey(event, row.name) : undefined}
      >
        <td>{row.name}<span class="sr-only">{rankLabel(copy.timeline.relative_rank, row.value)}</span></td>
        <td class="money">{row.cost}</td>
        <td class="money">{row.avg_per_session}</td>
        <td>{count(row.sessions)}</td>
        <td class="muted-cell">{row.tool_mix}</td>
      </tr>
    {:else}
      <tr><td colspan="5" class="empty-cell">{copy.empty.no_project_rows}</td></tr>
    {/each}
  </tbody>
</table>
