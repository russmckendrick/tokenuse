<script lang="ts">
  import { count } from '../../format';
  import type { CopyDeck, SessionMetric } from '../../types';
  import RankBar from '../RankBar.svelte';

  export let rows: SessionMetric[] = [];
  export let copy: CopyDeck;
</script>

<table class="data-table session-table">
  <thead><tr><th></th><th>{copy.tables.date}</th><th>{copy.tables.project}</th><th>{copy.tables.cost}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr>
        <td><RankBar value={row.value} ariaLabel={`${row.project} ${copy.desktop.session_rank}`} /></td>
        <td>{row.date}</td>
        <td>{row.project}</td>
        <td class="money">{row.cost}</td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="5" class="empty-cell">{copy.empty.no_sessions}</td></tr>
    {/each}
  </tbody>
</table>
