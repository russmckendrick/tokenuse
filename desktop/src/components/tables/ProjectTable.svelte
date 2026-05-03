<script lang="ts">
  import { count } from '../../format';
  import type { CopyDeck, ProjectMetric } from '../../types';
  import RankBar from '../RankBar.svelte';

  export let rows: ProjectMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table project-table">
  <thead><tr><th></th><th>{copy.tables.project}</th><th>{copy.tables.cost}</th><th>{copy.tables.avg_per_session}</th><th>{copy.tables.sess}</th><th>{copy.tables.tools}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr>
        <td><RankBar value={row.value} ariaLabel={`${row.name} ${copy.desktop.rank}`} /></td>
        <td>{row.name}</td>
        <td class="money">{row.cost}</td>
        <td class="money">{row.avg_per_session}</td>
        <td>{count(row.sessions)}</td>
        <td class="muted-cell">{row.tool_mix}</td>
      </tr>
    {:else}
      <tr><td colspan="6" class="empty-cell">{copy.empty.no_project_rows}</td></tr>
    {/each}
  </tbody>
</table>
