<script lang="ts">
  import { area, curveMonotoneX, line, scaleLinear } from 'd3';

  export let values: number[] = [];
  export let ariaLabel = '';
  export let height = 34;

  const WIDTH = 200;
  const INSET = 3;

  $: clamped = values.map((v) => Math.max(0, Math.min(100, Number.isFinite(v) ? v : 0)));
  $: x = scaleLinear()
    .domain([0, Math.max(clamped.length - 1, 1)])
    .range([INSET, WIDTH - INSET]);
  $: y = scaleLinear().domain([0, 100]).range([height - INSET, INSET]);
  $: linePath =
    clamped.length === 1
      ? `M${INSET},${y(clamped[0])}L${WIDTH - INSET},${y(clamped[0])}`
      : line<number>()
          .x((_, i) => x(i))
          .y((v) => y(v))
          .curve(curveMonotoneX)(clamped) ?? '';
  $: areaPath =
    clamped.length === 1
      ? `M${INSET},${height - INSET}L${INSET},${y(clamped[0])}L${WIDTH - INSET},${y(clamped[0])}L${WIDTH - INSET},${height - INSET}Z`
      : area<number>()
          .x((_, i) => x(i))
          .y0(height - INSET)
          .y1((v) => y(v))
          .curve(curveMonotoneX)(clamped) ?? '';
  $: last = clamped[clamped.length - 1] ?? 0;
  $: lastX = clamped.length === 1 ? WIDTH / 2 : x(clamped.length - 1);
  // Tone of the most recent point — same thresholds as ScoreBar.
  $: tone = last >= 70 ? 'good' : last >= 40 ? 'warn' : 'bad';
</script>

{#if clamped.length > 0}
  <svg
    class={`sparkline ${tone}`}
    viewBox={`0 0 ${WIDTH} ${height}`}
    preserveAspectRatio="none"
    role="img"
    aria-label={ariaLabel}
  >
    <line class="spark-baseline" x1={INSET} x2={WIDTH - INSET} y1={height - INSET} y2={height - INSET}
    ></line>
    <path class="spark-area" d={areaPath}></path>
    <path class="spark-line" d={linePath}></path>
    <circle class="spark-dot" cx={lastX} cy={y(last)} r="2.5"></circle>
  </svg>
{/if}

<style>
  svg {
    display: block;
    width: 100%;
  }

  .spark-baseline {
    stroke: var(--chart-grid);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .spark-area {
    opacity: 0.14;
  }

  .spark-line {
    fill: none;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .sparkline.good .spark-area,
  .sparkline.good .spark-dot {
    fill: var(--color-tertiary);
  }

  .sparkline.good .spark-line {
    stroke: var(--color-tertiary);
  }

  .sparkline.warn .spark-area,
  .sparkline.warn .spark-dot {
    fill: var(--color-warning);
  }

  .sparkline.warn .spark-line {
    stroke: var(--color-warning);
  }

  .sparkline.bad .spark-area,
  .sparkline.bad .spark-dot {
    fill: var(--color-error);
  }

  .sparkline.bad .spark-line {
    stroke: var(--color-error);
  }
</style>
