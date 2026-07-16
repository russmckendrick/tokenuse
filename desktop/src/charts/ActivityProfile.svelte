<script lang="ts">
  import { scaleLinear } from 'd3';

  export let matrix: number[][] = [];
  export let ariaLabel = '';
  export let weekdayLabel = '';
  export let weekendLabel = '';
  export let emptyLabel = '';

  const WIDTH = 640;
  const HEIGHT = 190;
  const LEFT = 38;
  const RIGHT = 10;
  const TOP = 12;
  const BOTTOM = 26;

  $: weekdays = Array.from({ length: 24 }, (_, hour) => matrix.slice(0, 5).reduce((sum, row) => sum + (row[hour] ?? 0), 0));
  $: weekends = Array.from({ length: 24 }, (_, hour) => matrix.slice(5, 7).reduce((sum, row) => sum + (row[hour] ?? 0), 0));
  $: max = Math.max(1, ...weekdays, ...weekends);
  $: y = scaleLinear().domain([0, max]).nice(3).range([HEIGHT - BOTTOM, TOP]);
  $: ticks = y.ticks(3);
  $: slot = (WIDTH - LEFT - RIGHT) / 24;
  $: barWidth = Math.max(2, slot * 0.34);
</script>

<div class="profile" role="img" aria-label={ariaLabel}>
  {#if matrix.length}
    <div class="legend" aria-hidden="true"><span><i class="weekday"></i>{weekdayLabel}</span><span><i class="weekend"></i>{weekendLabel}</span></div>
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
      {#each ticks as tick}
        <line class="grid" x1={LEFT} x2={WIDTH - RIGHT} y1={y(tick)} y2={y(tick)}></line>
      {/each}
      {#each weekdays as value, hour}
        <rect class="weekday-bar" x={LEFT + hour * slot + slot / 2 - barWidth} y={y(value)} width={barWidth} height={Math.max(1, HEIGHT - BOTTOM - y(value))}><title>{`${weekdayLabel} · ${hour}:00 · ${value}`}</title></rect>
        <rect class="weekend-bar" x={LEFT + hour * slot + slot / 2} y={y(weekends[hour])} width={barWidth} height={Math.max(1, HEIGHT - BOTTOM - y(weekends[hour]))}><title>{`${weekendLabel} · ${hour}:00 · ${weekends[hour]}`}</title></rect>
        {#if hour % 3 === 0}<text class="axis-label" x={LEFT + hour * slot + slot / 2} y={HEIGHT - 8} text-anchor="middle">{hour}:00</text>{/if}
      {/each}
    </svg>
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}
</div>

<style>
  .profile {
    width: 100%;
  }

  .legend {
    display: flex;
    justify-content: center;
    gap: var(--space-xl);
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .legend i {
    width: 20px;
    height: 7px;
  }

  .weekday,
  .weekday-bar {
    fill: var(--chart-series-3);
    background: var(--chart-series-3);
  }

  .weekend,
  .weekend-bar {
    fill: var(--color-error);
    background: var(--color-error);
  }

  svg {
    display: block;
    width: 100%;
    height: 190px;
  }

  .grid {
    stroke: var(--chart-grid);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .axis-label {
    fill: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 9px;
  }

  .chart-empty {
    min-height: 190px;
    display: grid;
    place-items: center;
    color: var(--color-muted-2);
    font-size: var(--text-body);
  }
</style>
