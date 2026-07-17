<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, ModelMetric } from '../../types';

  export let rows: ModelMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table model-table">
  <thead><tr><th>{copy.tables.model}</th><th>{copy.tables.cost}</th><th>{copy.tables.cache}</th><th>{copy.tables.cache_rate}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr class="rank-row" style:--rank-fill={`${rankPercent(row.value)}%`} title={rankLabel(copy.timeline.relative_rank, row.value)}>
        <td>{row.name}<span class="sr-only">{rankLabel(copy.timeline.relative_rank, row.value)}</span></td>
        <td class="money">{row.cost}</td>
        <td>{row.cache}</td>
        <td>{row.cache_rate}</td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="5" class="empty-cell">{copy.empty.no_models}</td></tr>
    {/each}
  </tbody>
</table>
