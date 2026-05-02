<script lang="ts">
  import { area, curveMonotoneX, line, scaleLinear } from 'd3';

  export let buckets: number[] = [];
  export let active = false;
  export let tone = 'cyan';
  export let ariaLabel = '24 hour activity';

  const chartWidth = 420;
  const chartHeight = 74;
  const inset = 4;
  const baseline = 56;
  const barBottom = 70;

  const tones: Record<string, { accent: string; fill: string; floor: string }> = {
    orange: { accent: '#ff8f40', fill: 'rgba(255, 143, 64, 0.18)', floor: '#b66d42' },
    magenta: { accent: '#f05af2', fill: 'rgba(240, 90, 242, 0.16)', floor: '#9953a8' },
    blue: { accent: '#62a6ff', fill: 'rgba(98, 166, 255, 0.16)', floor: '#5275aa' },
    green: { accent: '#4cf2a0', fill: 'rgba(76, 242, 160, 0.15)', floor: '#4aa879' },
    cyan: { accent: '#4df3e8', fill: 'rgba(77, 243, 232, 0.15)', floor: '#4aa6a9' },
    yellow: { accent: '#ffd60a', fill: 'rgba(255, 214, 10, 0.14)', floor: '#a99332' }
  };

  $: values = buckets.length ? buckets : Array.from({ length: 24 }, () => 0);
  $: maxValue = Math.max(1, ...values);
  $: palette = tones[tone] ?? tones.cyan;
  $: xScale = scaleLinear().domain([0, Math.max(values.length - 1, 1)]).range([inset, chartWidth - inset]);
  $: dense = values.length > 48;
  $: bucketSpan = (chartWidth - inset * 2) / Math.max(values.length, 1);
  $: tickWidth = Math.max(dense ? 1 : 2, Math.min(dense ? 5 : 12, bucketSpan * (dense ? 0.78 : 0.56)));
  $: yScale = scaleLinear().domain([0, maxValue]).range([baseline, 8]);
  $: trendLine =
    line<number>()
      .x((_, index) => bucketX(index))
      .y((value) => yScale(normalize(value)))
      .curve(curveMonotoneX)(values) ?? '';
  $: trendArea =
    area<number>()
      .x((_, index) => bucketX(index))
      .y0(baseline)
      .y1((value) => yScale(normalize(value)))
      .curve(curveMonotoneX)(values) ?? '';

  function normalize(value: number) {
    return Math.max(0, Math.min(maxValue, Number.isFinite(value) ? value : 0));
  }

  function bucketX(index: number) {
    return values.length <= 1 ? chartWidth / 2 : xScale(index);
  }

  function tickX(index: number) {
    return Math.max(inset, Math.min(chartWidth - inset - tickWidth, bucketX(index) - tickWidth / 2));
  }

  function tickY(value: number) {
    if (!active || value <= 0) return barBottom - 1;
    return barBottom - Math.max(3, Math.min(12, 3 + (normalize(value) / maxValue) * 9));
  }

  function tickHeight(value: number) {
    if (!active || value <= 0) return 1;
    return Math.max(3, barBottom - tickY(value));
  }
</script>

<svg
  class:quiet={!active}
  style={`--chart-accent: ${palette.accent}; --chart-fill: ${palette.fill}; --chart-floor: ${palette.floor}`}
  viewBox={`0 0 ${chartWidth} ${chartHeight}`}
  preserveAspectRatio="none"
  role="img"
  aria-label={ariaLabel}
>
  <line class="guide top-guide" x1={inset} x2={chartWidth - inset} y1="8" y2="8"></line>
  <line class="guide middle-guide" x1={inset} x2={chartWidth - inset} y1={baseline} y2={baseline}></line>
  <line class="guide floor-guide" x1={inset} x2={chartWidth - inset} y1={barBottom} y2={barBottom}></line>

  {#if active}
    <path class="trend-area" d={trendArea}></path>
    <path class="trend-line" d={trendLine}></path>
  {/if}

  {#each values as value, index}
    <rect
      class="activity-tick"
      class:empty={!active || value <= 0}
      x={tickX(index)}
      y={tickY(value)}
      width={tickWidth}
      height={tickHeight(value)}
    ></rect>
  {/each}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 72px;
  }

  .guide {
    stroke: #414866;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .top-guide {
    opacity: 0.42;
  }

  .middle-guide {
    stroke-dasharray: 2 5;
    opacity: 0.7;
  }

  .floor-guide {
    stroke: var(--chart-floor);
  }

  .trend-area {
    fill: var(--chart-fill);
  }

  .trend-line {
    fill: none;
    stroke: var(--chart-accent);
    stroke-width: 2.5;
    vector-effect: non-scaling-stroke;
  }

  .activity-tick {
    fill: #4df3e8;
    opacity: 0.74;
  }

  .activity-tick.empty {
    fill: #414866;
    opacity: 0.38;
  }

  svg.quiet .floor-guide {
    stroke: #6e7492;
  }
</style>
