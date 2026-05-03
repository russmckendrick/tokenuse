<script lang="ts">
  import { count } from '../../format';
  import type { CopyDeck, ModelMetric } from '../../types';
  import RankBar from '../RankBar.svelte';

  export let rows: ModelMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table model-table">
  <thead><tr><th></th><th>{copy.tables.model}</th><th>{copy.tables.cost}</th><th>{copy.tables.cache}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr>
        <td><RankBar value={row.value} ariaLabel={`${row.name} ${copy.desktop.rank}`} /></td>
        <td>{row.name}</td>
        <td class="money">{row.cost}</td>
        <td>{row.cache}</td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="5" class="empty-cell">{copy.empty.no_models}</td></tr>
    {/each}
  </tbody>
</table>
