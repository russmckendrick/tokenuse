<script lang="ts">
  export let matrix: number[][] = [];
  export let dayLabels: string[] = [];
  export let ariaLabel = '';
  export let emptyLabel = '';
  export let lessLabel = '';
  export let moreLabel = '';

  const CELL_W = 34;
  const CELL_H = 29;
  const GAP = 3;
  const LABEL_W = 48;
  const LABEL_H = 24;

  $: hours = matrix[0]?.length ?? 24;
  $: width = LABEL_W + hours * (CELL_W + GAP);
  $: height = LABEL_H + matrix.length * (CELL_H + GAP);
  $: max = matrix.reduce((outer, row) => Math.max(outer, ...row), 0);

  /* Single-hue intensity ramp on the panel accent, matching CommitGrid. */
  const RAMP = [
    'var(--color-bar-empty)',
    'color-mix(in srgb, var(--color-primary) 28%, var(--color-neutral))',
    'color-mix(in srgb, var(--color-primary) 52%, var(--color-neutral))',
    'color-mix(in srgb, var(--color-primary) 76%, var(--color-neutral))',
    'var(--color-primary)'
  ];

  function level(value: number): number {
    if (!value || !max) return 0;
    const ratio = value / max;
    if (ratio <= 0.25) return 1;
    if (ratio <= 0.5) return 2;
    if (ratio <= 0.75) return 3;
    return 4;
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
          <rect x={LABEL_W + hour * (CELL_W + GAP)} y={LABEL_H + day * (CELL_H + GAP)} width={CELL_W} height={CELL_H} rx="3" fill={RAMP[level(value)]}>
            <title>{`${dayLabels[day] ?? ''} ${String(hour).padStart(2, '0')}:00 · ${value.toLocaleString()}`}</title>
          </rect>
        {/each}
      {/each}
    </svg>
    <div class="legend" aria-hidden="true">
      <span>{lessLabel}</span>
      {#each RAMP as fill (fill)}
        <i style={`background: ${fill}`}></i>
      {/each}
      <span>{moreLabel}</span>
    </div>
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

  .legend {
    display: flex;
    align-items: center;
    gap: 3px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .legend i {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex: 0 0 auto;
  }

  .legend span {
    font-family: var(--font-ui);
    font-size: 10px;
    color: var(--color-muted-2);
    margin: 0 4px;
    white-space: nowrap;
  }

  .chart-empty {
    min-height: 230px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-size: var(--text-body);
  }
</style>
