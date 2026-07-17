<script lang="ts">
  import { scaleLinear } from 'd3';
  import type { StackedDayMetric } from '../types';
  import { toolColor } from './colors';

  export let rows: StackedDayMetric[] = [];
  export let ariaLabel = '';
  export let emptyLabel = '';

  const WIDTH = 640;
  const HEIGHT = 150;
  const LABEL_H = 14;
  const PLOT_H = HEIGHT - LABEL_H;

  let hovered: number | null = null;

  $: totals = rows.map((row) => row.segments.reduce((sum, segment) => sum + segment.amount, 0));
  $: maxTotal = totals.reduce((max, total) => Math.max(max, total), 0);
  $: y = scaleLinear().domain([0, Math.max(1, maxTotal)]).range([0, PLOT_H - 6]);
  $: slot = rows.length ? WIDTH / rows.length : WIDTH;
  $: barWidth = Math.max(3, Math.min(34, slot * 0.62));
  $: labelStep = Math.max(1, Math.ceil(rows.length / 12));

  type Slice = { x: number; y: number; height: number; color: string };

  function slices(row: StackedDayMetric, index: number): Slice[] {
    const x = slot * index + (slot - barWidth) / 2;
    let cursor = PLOT_H;
    return row.segments.map((segment, segmentIdx) => {
      const height = y(segment.amount);
      cursor -= height;
      return { x, y: cursor, height: Math.max(0, height - 0.6), color: toolColor(segment.tool, segmentIdx) };
    });
  }
</script>

<div class="stacked-bars" role="img" aria-label={ariaLabel}>
  {#if rows.length && maxTotal > 0}
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
    <line x1="0" x2={WIDTH} y1={PLOT_H} y2={PLOT_H} class="baseline" />
    {#each rows as row, index}
      {#each slices(row, index) as slice}
        <rect
          x={slice.x}
          y={slice.y}
          width={barWidth}
          height={slice.height}
          fill={slice.color}
          opacity={hovered === null || hovered === index ? 1 : 0.35}
        />
      {/each}
      <rect
        class="hit"
        x={slot * index}
        y="0"
        width={slot}
        height={PLOT_H}
        role="presentation"
        onmouseenter={() => (hovered = index)}
        onmouseleave={() => (hovered = null)}
      />
      {#if index % labelStep === 0}
        <!-- Clamp so edge labels stay inside the viewBox instead of clipping. -->
        <text
          x={Math.min(Math.max(slot * index + slot / 2, 16), WIDTH - 16)}
          y={HEIGHT - 3}
          text-anchor="middle"
          class="axis-label"
        >
          {row.day}
        </text>
      {/if}
    {/each}
    </svg>
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}

  {#if hovered !== null && rows[hovered]}
    <div class="chart-tooltip">
      <strong>{rows[hovered].day}</strong>
      <span class="mono">{rows[hovered].total_cost}</span>
      {#each rows[hovered].segments as segment, segmentIdx}
        <span class="tooltip-row">
          <i style={`background:${toolColor(segment.tool, segmentIdx)}`} aria-hidden="true"></i>
          {segment.tool_label}
          <em class="mono">{segment.cost}</em>
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .stacked-bars {
    position: relative;
    width: 100%;
  }

  svg {
    display: block;
    width: 100%;
    height: 150px;
  }

  .chart-empty {
    min-height: 150px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 12px;
  }

  .baseline {
    stroke: var(--chart-grid);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .hit {
    fill: transparent;
  }

  .axis-label {
    fill: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 10px;
  }

  rect {
    transition: opacity var(--motion-fast) var(--ease-standard);
  }
</style>
