<script lang="ts">
  import { count, rankLabel, rankPercent } from '../../format';
  import type { CopyDeck, ModelMetric } from '../../types';

  export let rows: ModelMetric[] = [];
  export let copy: CopyDeck;
  export let openModel: ((id: string, name: string) => void) | null = null;

  function handleRowKey(event: KeyboardEvent, row: ModelMetric) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openModel?.(row.canonical_id, row.name);
    }
  }
</script>

<table class="data-table model-table">
  <thead><tr><th>{copy.tables.model}</th><th>{copy.tables.cost}</th><th>{copy.tables.cache}</th><th>{copy.tables.cache_rate}</th><th>{copy.tables.calls}</th></tr></thead>
  <tbody>
    {#each rows as row}
      <tr
        class="rank-row"
        class:click-row={Boolean(openModel)}
        style:--rank-fill={`${rankPercent(row.value)}%`}
        title={rankLabel(copy.timeline.relative_rank, row.value)}
        tabindex={openModel ? 0 : undefined}
        onclick={openModel ? () => openModel?.(row.canonical_id, row.name) : undefined}
        onkeydown={openModel ? (event) => handleRowKey(event, row) : undefined}
      >
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
