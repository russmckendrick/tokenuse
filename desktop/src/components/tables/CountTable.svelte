<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, CountMetric } from '../../types';

  export let rows: CountMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table count-table">
  <thead><tr><th>{copy.tables.name}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr class="rank-row" style:--rank-fill={`${rankPercent(row.value)}%`} title={rankLabel(copy.timeline.relative_rank, row.value)}>
        <td>{row.name}<span class="sr-only">{rankLabel(copy.timeline.relative_rank, row.value)}</span></td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="2" class="empty-cell">{copy.empty.no_rows}</td></tr>
    {/each}
  </tbody>
</table>
