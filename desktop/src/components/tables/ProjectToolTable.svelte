<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, ProjectToolMetric } from '../../types';

  export let rows: ProjectToolMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table project-tool-table">
  <thead><tr><th>{copy.tables.project}</th><th>{copy.tables.tool}</th><th>{copy.tables.cost}</th><th>{copy.tables.calls}</th><th>{copy.tables.sess}</th><th>{copy.tables.avg_per_session}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr class="rank-row" style:--rank-fill={`${rankPercent(row.value)}%`} title={rankLabel(copy.timeline.relative_rank, row.value)}>
        <td>{row.project}<span class="sr-only">{rankLabel(copy.timeline.relative_rank, row.value)}</span></td>
        <td>{row.tool}</td>
        <td class="money">{row.cost}</td>
        <td>{count(row.calls)}</td>
        <td>{count(row.sessions)}</td>
        <td class="money">{row.avg_per_session}</td>
      </tr>
    {:else}
      <tr><td colspan="6" class="empty-cell">{copy.empty.no_project_tool_rows}</td></tr>
    {/each}
  </tbody>
</table>
