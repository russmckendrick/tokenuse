<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, SessionMetric } from '../../types';

  export let rows: SessionMetric[] = [];
  export let copy: CopyDeck;
  export let openSession: ((key: string) => void) | null = null;

  function linked(row: SessionMetric): boolean {
    return Boolean(openSession && row.key);
  }

  function handleRowKey(event: KeyboardEvent, key: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openSession?.(key);
    }
  }
</script>

<table class="data-table session-table">
  <thead><tr><th>{copy.tables.date}</th><th>{copy.tables.project}</th><th>{copy.tables.cost}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr
        class="rank-row"
        class:click-row={linked(row)}
        style:--rank-fill={`${rankPercent(row.value)}%`}
        title={rankLabel(copy.timeline.relative_rank, row.value)}
        tabindex={linked(row) ? 0 : undefined}
        onclick={linked(row) ? () => openSession?.(row.key) : undefined}
        onkeydown={linked(row) ? (event) => handleRowKey(event, row.key) : undefined}
      >
        <td>{row.date}</td>
        <td>{row.project}<span class="sr-only">{rankLabel(copy.timeline.relative_rank, row.value)}</span></td>
        <td class="money">{row.cost}</td>
        <td>{count(row.calls)}</td>
      </tr>
    {:else}
      <tr><td colspan="4" class="empty-cell">{copy.empty.no_sessions}</td></tr>
    {/each}
  </tbody>
</table>
