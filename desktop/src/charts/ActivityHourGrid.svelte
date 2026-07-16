<script lang="ts">
  export let matrix: number[][] = [];
  export let dayLabels: string[] = [];
  export let ariaLabel = '';
  export let emptyLabel = '';

  const CELL_W = 34;
  const CELL_H = 29;
  const GAP = 3;
  const LABEL_W = 48;
  const LABEL_H = 24;

  $: hours = matrix[0]?.length ?? 24;
  $: width = LABEL_W + hours * (CELL_W + GAP);
  $: height = LABEL_H + matrix.length * (CELL_H + GAP);
  $: max = matrix.reduce((outer, row) => Math.max(outer, ...row), 0);

  function cellColor(value: number): string {
    if (!value || !max) return 'var(--color-bar-empty)';
    const ratio = value / max;
    if (ratio < 0.25) return 'var(--chart-series-6)';
    if (ratio < 0.5) return 'var(--chart-series-5)';
    if (ratio < 0.75) return 'var(--color-primary)';
    return 'var(--color-warning)';
  }

  function compactCount(value: number): string {
    if (value >= 1_000_000) return `${Math.round(value / 100_000) / 10}M`;
    if (value >= 1_000) return `${Math.round(value / 100) / 10}K`;
    return String(value);
  }
</script>

<div class="hour-grid" role="img" aria-label={ariaLabel}>
  {#if max > 0}
    <svg viewBox={`0 0 ${width} ${height}`} style={`min-width: ${width}px`}>
      {#each Array.from({ length: hours }) as _, hour}
        <text class="axis-label" x={LABEL_W + hour * (CELL_W + GAP) + CELL_W / 2} y="14" text-anchor="middle">{hour}</text>
      {/each}
      {#each matrix as row, day}
        <text class="day-label" x={LABEL_W - 8} y={LABEL_H + day * (CELL_H + GAP) + CELL_H / 2 + 4} text-anchor="end">{dayLabels[day] ?? ''}</text>
        {#each row as value, hour}
          <rect x={LABEL_W + hour * (CELL_W + GAP)} y={LABEL_H + day * (CELL_H + GAP)} width={CELL_W} height={CELL_H} rx="3" fill={cellColor(value)}>
            <title>{`${dayLabels[day] ?? ''} ${String(hour).padStart(2, '0')}:00 · ${value.toLocaleString()}`}</title>
          </rect>
          {#if value > 0}
            <text class="cell-value" x={LABEL_W + hour * (CELL_W + GAP) + CELL_W / 2} y={LABEL_H + day * (CELL_H + GAP) + CELL_H / 2 + 4} text-anchor="middle">{compactCount(value)}</text>
          {/if}
        {/each}
      {/each}
    </svg>
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}
</div>

<style>
  .hour-grid {
    width: 100%;
    overflow-x: auto;
    padding-bottom: 4px;
  }

  svg {
    display: block;
    width: 100%;
  }

  .axis-label,
  .day-label {
    fill: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 10px;
  }

  .cell-value {
    fill: var(--color-on-surface);
    font-family: var(--font-mono);
    font-size: 9px;
    pointer-events: none;
  }

  .chart-empty {
    min-height: 230px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-size: var(--text-body);
  }
</style>
