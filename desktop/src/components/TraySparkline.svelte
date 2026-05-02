<script lang="ts">
  import { area, curveMonotoneX, line, scaleLinear } from 'd3';
  import type { ActivityMetric } from '../types';

  export let points: ActivityMetric[] = [];

  const chartWidth = 280;
  const chartHeight = 36;
  const inset = 3;

  $: maxCalls = points.reduce((max, point) => Math.max(max, point.calls), 0);
  $: peak = points.length
    ? points.reduce((best, point) => (point.value > best.value ? point : best), points[0])
    : null;
  $: xScale = scaleLinear().domain([0, Math.max(points.length - 1, 1)]).range([inset, chartWidth - inset]);
  $: spendScale = scaleLinear().domain([0, 100]).range([chartHeight - 17, inset]);
  $: callsScale = scaleLinear().domain([0, Math.max(maxCalls, 1)]).range([chartHeight - 2, chartHeight - 15]);
  $: dense = points.length > 48;
  $: bucketSpan = (chartWidth - inset * 2) / Math.max(points.length, 1);
  $: barWidth = Math.max(1, Math.min(dense ? 4 : 8, bucketSpan * (dense ? 0.8 : 0.58)));
  $: callsLine =
    line<ActivityMetric>()
      .x((_, index) => bucketX(index))
      .y((point) => callsScale(point.calls))
      .curve(curveMonotoneX)(points) ?? '';
  $: callsArea =
    area<ActivityMetric>()
      .x((_, index) => bucketX(index))
      .y0(chartHeight - 2)
      .y1((point) => callsScale(point.calls))
      .curve(curveMonotoneX)(points) ?? '';

  function clampPercent(value: number) {
    return Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  }

  function bucketX(index: number) {
    return points.length <= 1 ? chartWidth / 2 : xScale(index);
  }

  function barX(index: number) {
    return Math.max(inset, Math.min(chartWidth - inset - barWidth, bucketX(index) - barWidth / 2));
  }

  function spendY(value: number) {
    if (value <= 0) return chartHeight - 17;
    return spendScale(clampPercent(value));
  }

  function spendHeight(value: number) {
    if (value <= 0) return 1;
    return Math.max(1, chartHeight - 17 - spendScale(clampPercent(value)));
  }
</script>

<svg viewBox={`0 0 ${chartWidth} ${chartHeight}`} preserveAspectRatio="none" aria-hidden="true">
  <line class="spark-guide" x1={inset} x2={chartWidth - inset} y1={chartHeight - 17} y2={chartHeight - 17}></line>
  {#each points as point, index}
    <rect
      class="spark-bar"
      class:peak={point === peak}
      x={barX(index)}
      y={spendY(point.value)}
      width={barWidth}
      height={spendHeight(point.value)}
    ></rect>
  {/each}
  <path class="spark-area" d={callsArea}></path>
  <path class="spark-line" d={callsLine}></path>
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    height: 36px;
  }

  .spark-guide {
    stroke: #414866;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .spark-area {
    fill: rgba(77, 243, 232, 0.13);
  }

  .spark-line {
    fill: none;
    stroke: #4df3e8;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .spark-bar {
    fill: #ff8f40;
    opacity: 0.76;
  }

  .spark-bar.peak {
    fill: #ff5f6d;
  }
</style>
