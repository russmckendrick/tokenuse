<script lang="ts">
  import { count } from '../../format';
  import type { CopyDeck, ProjectToolMetric } from '../../types';
  import RankBar from '../RankBar.svelte';

  export let rows: ProjectToolMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table project-tool-table">
  <thead><tr><th></th><th>{copy.tables.project}</th><th>{copy.tables.tool}</th><th>{copy.tables.cost}</th><th>{copy.tables.calls}</th><th>{copy.tables.sess}</th><th>{copy.tables.avg_per_session}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr>
        <td><RankBar value={row.value} ariaLabel={`${row.project} ${row.tool} ${copy.desktop.rank}`} /></td>
        <td>{row.project}</td>
        <td>{row.tool}</td>
        <td class="money">{row.cost}</td>
        <td>{count(row.calls)}</td>
        <td>{count(row.sessions)}</td>
        <td class="money">{row.avg_per_session}</td>
      </tr>
    {:else}
      <tr><td colspan="7" class="empty-cell">{copy.empty.no_project_tool_rows}</td></tr>
    {/each}
  </tbody>
</table>
