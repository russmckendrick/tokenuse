<script lang="ts">
  import { count } from '../../format';
  import type { CopyDeck, CountMetric } from '../../types';
  import RankBar from '../RankBar.svelte';

  export let rows: CountMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table count-table">
  <thead><tr><th></th><th>{copy.tables.name}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr>
        <td><RankBar value={row.value} ariaLabel={`${row.name} ${copy.desktop.rank}`} /></td>
        <td>{row.name}</td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="3" class="empty-cell">{copy.empty.no_rows}</td></tr>
    {/each}
  </tbody>
</table>
