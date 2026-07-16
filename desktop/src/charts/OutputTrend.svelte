<script lang="ts">
  import { curveMonotoneX, line, scaleLinear } from 'd3';
  import type { CountMetric } from '../types';

  export let rows: CountMetric[] = [];
  export let ariaLabel = '';
  export let emptyLabel = '';
  export let barsLabel = '';
  export let averageLabel = '';
  export let tickLabel: (row: CountMetric) => string = (row) => row.name.slice(5);
  export let detailLabel: (row: CountMetric, average: number) => string = () => '';

  const WIDTH = 920;
  const HEIGHT = 230;
  const LEFT = 52;
  const RIGHT = 18;
  const TOP = 20;
  const BOTTOM = 34;

  let selected = -1;
  let loadedSeries = '';

  $: points = [...rows].reverse();
  $: max = Math.max(1, ...points.map((row) => row.calls));
  $: plotWidth = WIDTH - LEFT - RIGHT;
  $: plotHeight = HEIGHT - TOP - BOTTOM;
  $: slot = points.length ? plotWidth / points.length : plotWidth;
  $: barWidth = Math.max(2, Math.min(42, slot * 0.58));
  $: y = scaleLinear().domain([0, max]).nice(4).range([TOP + plotHeight, TOP]);
  $: ticks = y.ticks(4);
  $: rolling = points.map((row, index) => {
    const window = points.slice(Math.max(0, index - 2), index + 1);
    return window.reduce((sum, item) => sum + item.calls, 0) / window.length;
  });
  $: averagePath =
    line<number>()
      .x((_, index) => LEFT + slot * index + slot / 2)
      .y((value) => y(value))
      .curve(curveMonotoneX)(rolling) ?? '';
  $: labelStep = Math.max(1, Math.ceil(points.length / 12));
  $: seriesKey = points.length
    ? `${points.length}|${points[0].name}|${points[points.length - 1].name}`
    : '';
  $: if (seriesKey !== loadedSeries) {
    loadedSeries = seriesKey;
    selected = points.length - 1;
  }
  $: selectedRow = points[selected] ?? null;
  $: selectedAverage = rolling[selected] ?? 0;
</script>

<div class="output-trend-chart" role="img" aria-label={ariaLabel}>
  {#if points.length}
    <div class="chart-legend" aria-hidden="true">
      <span><i class="bar-key"></i>{barsLabel}</span>
      <span><i class="line-key"></i>{averageLabel}</span>
    </div>
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
      {#each ticks as tick}
        <line class="grid" x1={LEFT} x2={WIDTH - RIGHT} y1={y(tick)} y2={y(tick)}></line>
        <text class="axis-label" x={LEFT - 8} y={y(tick) + 3} text-anchor="end">{Math.round(tick).toLocaleString()}</text>
      {/each}
      {#each points as row, index (row.name)}
        <rect
          class="bar"
          class:selected={index === selected}
          x={LEFT + slot * index + (slot - barWidth) / 2}
          y={y(row.calls)}
          width={barWidth}
          height={row.calls ? Math.max(1, TOP + plotHeight - y(row.calls)) : 0}
          role="button"
          tabindex="0"
          onclick={() => (selected = index)}
          onkeydown={(event) => (event.key === 'Enter' || event.key === ' ') && (selected = index)}
        >
          <title>{detailLabel(row, rolling[index])}</title>
        </rect>
        {#if index % labelStep === 0 || index === points.length - 1}
          <text class="axis-label" x={LEFT + slot * index + slot / 2} y={HEIGHT - 10} text-anchor="middle">{tickLabel(row)}</text>
        {/if}
      {/each}
      <path class="average-line" d={averagePath}></path>
      {#each rolling as value, index}
        {#if index === selected}<circle class="average-dot" cx={LEFT + slot * index + slot / 2} cy={y(value)} r="4"></circle>{/if}
      {/each}
    </svg>
    {#if selectedRow}
      <div class="selected-day">
        <strong class="mono">{selectedRow.name}</strong>
        <span>{detailLabel(selectedRow, selectedAverage)}</span>
      </div>
    {/if}
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}
</div>

<style>
  .output-trend-chart {
    width: 100%;
  }

  .chart-legend {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-xl);
    margin-bottom: 4px;
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .chart-legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .chart-legend i {
    display: inline-block;
    width: 18px;
  }

  .bar-key {
    height: 8px;
    background: var(--chart-series-5);
  }

  .line-key {
    height: 2px;
    background: var(--color-warning);
  }

  svg {
    display: block;
    width: 100%;
    height: 250px;
  }

  .grid {
    stroke: var(--chart-grid);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .axis-label {
    fill: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 10px;
  }

  .bar {
    fill: var(--chart-series-5);
    opacity: 0.72;
    cursor: pointer;
    transition: opacity var(--motion-fast) var(--ease-standard);
  }

  .bar:hover,
  .bar.selected {
    opacity: 1;
  }

  .bar:focus-visible {
    outline: none;
    stroke: var(--color-secondary);
    stroke-width: 2;
  }

  .average-line {
    fill: none;
    stroke: var(--color-warning);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    pointer-events: none;
  }

  .average-dot {
    fill: var(--color-warning);
    stroke: var(--color-neutral);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    pointer-events: none;
  }

  .selected-day {
    display: flex;
    align-items: baseline;
    gap: var(--space-lg);
    min-height: 28px;
    padding: 8px 0 0 var(--space-xl);
    border-top: 1px solid var(--color-border-row);
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .selected-day strong {
    color: var(--color-on-surface);
  }

  .chart-empty {
    min-height: 250px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-size: var(--text-body);
  }

  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
</style>
