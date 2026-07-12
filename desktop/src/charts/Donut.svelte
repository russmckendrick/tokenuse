<script lang="ts">
  import { arc, pie } from 'd3';
  import type { ShareMetric } from '../types';
  import { providerColor, seriesColor, toolColor } from './colors';

  export let rows: ShareMetric[] = [];
  export let centerLabel = '';
  export let colorBy: 'provider' | 'tool' | 'series' = 'series';
  export let ariaLabel = '';

  const SIZE = 132;
  const RADIUS = SIZE / 2;

  function color(row: ShareMetric, index: number): string {
    if (colorBy === 'provider') return providerColor(row.key);
    if (colorBy === 'tool') return toolColor(row.key, index);
    return seriesColor(index);
  }

  $: slices = pie<ShareMetric>()
    .value((row) => Math.max(0, row.share))
    .sort(null)(rows.filter((row) => row.share > 0));
  $: shape = arc<d3.PieArcDatum<ShareMetric>>()
    .innerRadius(RADIUS - 16)
    .outerRadius(RADIUS - 2)
    .cornerRadius(3)
    .padAngle(0.012);
</script>

<div class="donut" role="img" aria-label={ariaLabel}>
  <svg viewBox={`0 0 ${SIZE} ${SIZE}`} width={SIZE} height={SIZE}>
    <g transform={`translate(${RADIUS} ${RADIUS})`}>
      {#each slices as slice}
        <path d={shape(slice) ?? ''} fill={color(slice.data, slice.index)} />
      {/each}
    </g>
    <text x={RADIUS} y={RADIUS + 4} text-anchor="middle" class="center-label">{centerLabel}</text>
  </svg>

  <div class="legend">
    {#each rows as row, index}
      <div class="legend-row">
        <i style={`background:${color(row, index)}`} aria-hidden="true"></i>
        <span class="legend-label">{row.label}</span>
        <span class="mono legend-cost">{row.cost}</span>
        <span class="mono legend-share">{(row.share / 10).toFixed(row.share >= 100 ? 0 : 1)}%</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .donut {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: var(--space-xl);
  }

  svg {
    display: block;
  }

  .center-label {
    fill: var(--color-on-surface);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-size: 14px;
    font-weight: 700;
  }

  .legend {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .legend-row {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: var(--space-md);
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--color-on-surface);
  }

  .legend-row i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .legend-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .legend-cost {
    color: var(--color-warning);
    font-size: 12px;
  }

  .legend-share {
    color: var(--color-muted-2);
    font-size: 11px;
    min-width: 42px;
    text-align: right;
  }
</style>
