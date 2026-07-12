<script lang="ts">
  export let matrix: number[][] = [];
  export let dayLabels: string[] = [];
  export let ariaLabel = '';
  export let emptyLabel = '';

  const CELL = 18;
  const GAP = 1;
  const LABEL_W = 34;
  const LABEL_H = 14;

  $: hours = matrix[0]?.length ?? 24;
  $: width = LABEL_W + hours * (CELL + GAP);
  $: height = LABEL_H + matrix.length * (CELL + GAP);
  $: max = matrix.reduce(
    (outer, row) => row.reduce((inner, cell) => Math.max(inner, cell), outer),
    0
  );

  /* Stepped ramp: empty -> secondary -> primary -> error at saturation. */
  function cellColor(value: number): string {
    if (max === 0 || value === 0) return 'var(--color-bar-empty)';
    const ratio = value / max;
    if (ratio < 0.25) return 'var(--chart-series-3)';
    if (ratio < 0.55) return 'var(--chart-series-2)';
    if (ratio < 0.85) return 'var(--chart-series-1)';
    return 'var(--chart-peak)';
  }

  function cellOpacity(value: number): number {
    if (max === 0 || value === 0) return 1;
    const ratio = value / max;
    return 0.45 + Math.min(0.55, ratio * 0.75);
  }
</script>

<div class="heatmap" role="img" aria-label={ariaLabel}>
  {#if max > 0}
    <svg viewBox={`0 0 ${width} ${height}`} style={`aspect-ratio: ${width} / ${height};`}>
    {#each matrix as row, dayIdx}
      <text x={LABEL_W - 6} y={LABEL_H + dayIdx * (CELL + GAP) + CELL / 2 + 3} text-anchor="end" class="axis-label">
        {dayLabels[dayIdx] ?? ''}
      </text>
      {#each row as value, hourIdx}
        <rect
          x={LABEL_W + hourIdx * (CELL + GAP)}
          y={LABEL_H + dayIdx * (CELL + GAP)}
          width={CELL}
          height={CELL}
          rx="2"
          fill={cellColor(value)}
          opacity={cellOpacity(value)}
        >
          <title>{`${dayLabels[dayIdx] ?? ''} ${String(hourIdx).padStart(2, '0')}:00`}</title>
        </rect>
      {/each}
    {/each}
    {#each Array.from({ length: hours }) as _, hourIdx}
      {#if hourIdx % 3 === 0}
        <text
          x={LABEL_W + hourIdx * (CELL + GAP) + CELL / 2}
          y={LABEL_H - 4}
          text-anchor="middle"
          class="axis-label"
        >
          {String(hourIdx).padStart(2, '0')}
        </text>
      {/if}
    {/each}
    </svg>
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}
</div>

<style>
  .heatmap {
    width: 100%;
    overflow-x: auto;
  }

  svg {
    display: block;
    width: 100%;
    min-width: 420px;
  }

  .chart-empty {
    min-height: 147px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 12px;
  }

  .axis-label {
    fill: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 9px;
  }
</style>
