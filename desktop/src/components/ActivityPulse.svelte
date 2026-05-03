<script lang="ts">
  import { area, curveMonotoneX, line, scaleLinear } from 'd3';
  import { chartRefresh, staggeredReveal } from '../motion';
  import type { ActivityMetric, CopyDeck } from '../types';

  export let points: ActivityMetric[] = [];
  export let density: 'compact' | 'roomy' = 'roomy';
  export let copy: CopyDeck;

  const chartWidth = 640;
  const chartHeight = 116;
  const xInset = 10;
  const spendTop = 12;
  const spendBottom = 52;
  const callsTop = 70;
  const callsBottom = 108;

  let svgElement: SVGSVGElement | null = null;
  let hoverIndex: number | null = null;
  let pendingClientX: number | null = null;
  let hoverFrame: number | null = null;
  let lastChartKey = '';

  $: maxCalls = points.reduce((max, point) => Math.max(max, point.calls), 0);
  $: totalCalls = points.reduce((total, point) => total + point.calls, 0);
  $: high = points.length
    ? points.reduce((best, point) => (point.value > best.value ? point : best), points[0])
    : null;
  $: latest = points.length ? points[points.length - 1] : null;
  $: firstLabel = points[0]?.label ?? '-';
  $: lastLabel = latest?.label ?? '-';
  $: chartKey = `${points.length}:${firstLabel}:${lastLabel}`;
  $: dense = points.length > 192;
  $: domainEnd = Math.max(points.length - 1, 1);
  $: xScale = scaleLinear().domain([0, domainEnd]).range([xInset, chartWidth - xInset]);
  $: bucketSpan = (chartWidth - xInset * 2) / Math.max(points.length, 1);
  $: bucketBarWidth = Math.max(dense ? 1 : 2, Math.min(dense ? 5 : 14, bucketSpan * (dense ? 0.82 : 0.62)));
  $: spendScale = scaleLinear().domain([0, 100]).range([spendBottom, spendTop]);
  $: callsScale = scaleLinear().domain([0, Math.max(maxCalls, 1)]).range([callsBottom, callsTop]);
  $: callsLine =
    line<ActivityMetric>()
      .x((_, index) => bucketX(index))
      .y((point) => callsScale(point.calls))
      .curve(curveMonotoneX)(points) ?? '';
  $: callsArea =
    area<ActivityMetric>()
      .x((_, index) => bucketX(index))
      .y0(callsBottom)
      .y1((point) => callsScale(point.calls))
      .curve(curveMonotoneX)(points) ?? '';
  $: hoverPoint = hoverIndex === null ? null : points[hoverIndex] ?? null;
  $: hoverX = hoverIndex === null ? 0 : bucketX(hoverIndex);
  $: hoverBarX = hoverIndex === null ? 0 : barX(hoverIndex);
  $: if (chartKey !== lastChartKey) {
    lastChartKey = chartKey;
    clearHover();
  }

  function clampPercent(value: number) {
    return Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  }

  function bucketX(index: number) {
    return points.length <= 1 ? chartWidth / 2 : xScale(index);
  }

  function barX(index: number) {
    return Math.max(xInset, Math.min(chartWidth - xInset - bucketBarWidth, bucketX(index) - bucketBarWidth / 2));
  }

  function spendY(value: number) {
    if (value <= 0) return spendBottom - 1;
    return spendScale(clampPercent(value));
  }

  function spendHeight(value: number) {
    if (value <= 0) return 1;
    return Math.max(2, spendBottom - spendScale(clampPercent(value)));
  }

  function compactCount(value: number) {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
    return value.toLocaleString();
  }

  function handlePointerMove(event: PointerEvent) {
    if (!svgElement || !points.length) return;
    pendingClientX = event.clientX;

    if (hoverFrame !== null) return;
    hoverFrame = window.requestAnimationFrame(() => {
      hoverFrame = null;
      if (!svgElement || pendingClientX === null) return;

      const rect = svgElement.getBoundingClientRect();
      const localX = ((pendingClientX - rect.left) / rect.width) * chartWidth;
      const nextIndex = Math.max(0, Math.min(points.length - 1, Math.round(xScale.invert(localX))));
      if (nextIndex !== hoverIndex) {
        hoverIndex = nextIndex;
      }
    });
  }

  function clearHover() {
    if (hoverFrame !== null) {
      window.cancelAnimationFrame(hoverFrame);
      hoverFrame = null;
    }
    pendingClientX = null;
    hoverIndex = null;
  }
</script>

{#if points.length}
  <div class="activity-pulse" class:compact={density === 'compact'} use:staggeredReveal={{ selector: '.activity-chart, .pulse-meta span', y: 3 }}>
    <div class="activity-chart">
      <div class="pulse-label spend">{copy.timeline.spend}</div>
      <div class="pulse-label calls">{copy.timeline.calls}</div>
      {#key chartKey}
        <div class="chart-frame" use:chartRefresh={{ y: 2 }}>
          <svg
            bind:this={svgElement}
            viewBox={`0 0 ${chartWidth} ${chartHeight}`}
            preserveAspectRatio="none"
            role="img"
            aria-label={copy.timeline.activity_aria.replace('{first}', firstLabel).replace('{last}', lastLabel)}
            onpointermove={handlePointerMove}
            onpointerleave={clearHover}
          >
            <rect class="calls-band" x={xInset} y={callsTop - 2} width={chartWidth - xInset * 2} height={callsBottom - callsTop + 2}></rect>

            <line class="guide spend-guide" x1={xInset} x2={chartWidth - xInset} y1={spendTop} y2={spendTop}></line>
            <line class="guide middle-guide" x1={xInset} x2={chartWidth - xInset} y1={spendBottom} y2={spendBottom}></line>
            <line class="guide calls-guide" x1={xInset} x2={chartWidth - xInset} y1={callsBottom} y2={callsBottom}></line>

            {#each points as point, index}
              <rect
                class="spend-bar"
                class:empty={point.value === 0}
                class:peak={point === high}
                x={barX(index)}
                y={spendY(point.value)}
                width={bucketBarWidth}
                height={spendHeight(point.value)}
              >
                <title>{point.label} · {point.cost} · {point.calls.toLocaleString()} {copy.metrics.calls}</title>
              </rect>
            {/each}

            <path class="calls-area" d={callsArea}></path>
            <path class="calls-line" d={callsLine}></path>

            {#if hoverPoint}
              <line class="hover-line" x1={hoverX} x2={hoverX} y1={spendTop} y2={callsBottom}></line>
              <rect
                class="hover-bar"
                x={hoverBarX}
                y={spendY(hoverPoint.value)}
                width={bucketBarWidth}
                height={spendHeight(hoverPoint.value)}
              ></rect>
            {/if}
          </svg>

          {#if hoverPoint}
            <div class="pulse-tooltip" style={`left: ${Math.min(92, Math.max(8, (hoverX / chartWidth) * 100))}%`}>
              <strong>{hoverPoint.label}</strong>
              <span>{hoverPoint.cost}</span>
              <span>{hoverPoint.calls.toLocaleString()} {copy.metrics.calls}</span>
            </div>
          {/if}
        </div>
      {/key}
    </div>

    <div class="pulse-meta">
      <span>{copy.timeline.range} <strong>{firstLabel}</strong> {copy.timeline.to} <strong>{lastLabel}</strong></span>
      <span>{copy.timeline.high} <strong>{high?.label ?? '-'}</strong> <b>{high?.cost ?? '-'}</b></span>
      <span>{copy.timeline.latest} <b>{latest?.cost ?? '-'}</b></span>
      <span>{copy.timeline.calls} <strong>{compactCount(totalCalls)}</strong></span>
    </div>
  </div>
{:else}
  <div class="activity-empty">{copy.timeline.no_activity}</div>
{/if}

<style>
  .activity-pulse {
    height: 100%;
    min-height: 108px;
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: 6px;
  }

  .activity-pulse.compact {
    min-height: 86px;
  }

  .activity-chart {
    min-width: 0;
    min-height: 72px;
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    grid-template-rows: 1fr 1fr;
    gap: 2px 8px;
    align-items: center;
  }

  .pulse-label {
    font-weight: 800;
  }

  .pulse-label.spend {
    color: #ff8f40;
  }

  .pulse-label.calls {
    color: #7ebcff;
  }

  .chart-frame {
    position: relative;
    grid-column: 2;
    grid-row: 1 / -1;
    min-width: 0;
    height: 100%;
    min-height: 72px;
    overflow: hidden;
    background:
      linear-gradient(#292d42, #292d42) 0 50% / 100% 1px no-repeat,
      linear-gradient(90deg, rgba(77, 243, 232, 0.035), rgba(255, 143, 64, 0.035));
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 72px;
  }

  .guide {
    vector-effect: non-scaling-stroke;
    stroke: #414866;
    stroke-width: 1;
  }

  .calls-band {
    fill: rgba(98, 166, 255, 0.035);
  }

  .spend-guide {
    stroke: #414866;
    opacity: 0.55;
  }

  .middle-guide {
    stroke-dasharray: 2 4;
    opacity: 0.7;
  }

  .calls-guide {
    stroke: #414866;
    opacity: 0.85;
  }

  .spend-bar {
    fill: #ff8f40;
    opacity: 0.76;
  }

  .spend-bar.empty {
    fill: #414866;
    opacity: 0.42;
  }

  .spend-bar.peak {
    fill: #ff5f6d;
    opacity: 0.9;
  }

  .calls-area {
    fill: rgba(77, 243, 232, 0.12);
    stroke: none;
  }

  .calls-line {
    fill: none;
    stroke: #4df3e8;
    stroke-width: 2.5;
    vector-effect: non-scaling-stroke;
  }

  .hover-bar {
    fill: transparent;
    stroke: #ffd60a;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .hover-line {
    stroke: #ffd60a;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .pulse-tooltip {
    position: absolute;
    top: 4px;
    z-index: 2;
    display: grid;
    gap: 1px;
    min-width: 118px;
    padding: 5px 7px;
    color: #cbd4f2;
    background: #202438;
    border: 1px solid #ffd60a;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .pulse-tooltip strong,
  .pulse-tooltip span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pulse-tooltip strong {
    color: #ffd60a;
  }

  .pulse-tooltip span {
    color: #a1a7c3;
  }

  .pulse-meta {
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
    color: #a1a7c3;
    overflow: hidden;
  }

  .pulse-meta span {
    min-width: 0;
    white-space: nowrap;
  }

  .pulse-meta strong {
    color: #cbd4f2;
    font-weight: 700;
  }

  .pulse-meta b {
    color: #ffd60a;
  }

  .activity-empty {
    min-height: 92px;
    display: grid;
    place-items: center;
    color: #a1a7c3;
    border: 1px solid #292d42;
  }

  @media (prefers-reduced-motion: reduce) {
    .pulse-tooltip {
      transition: none;
    }
  }
</style>
