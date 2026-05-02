<script lang="ts">
  import { range, scaleLinear, scaleQuantize } from 'd3';

  export let value = 0;
  export let ariaLabel = 'relative rank';
  export let compact = false;

  const segmentCount = 12;
  const gap = 1;
  const colorScale = scaleQuantize<string>()
    .domain([0, 100])
    .range(['#62a6ff', '#7ebcff', '#f5cf6c', '#ffd60a', '#ff9c48', '#ff5f6d']);
  const opacityScale = scaleLinear().domain([0, segmentCount - 1]).range([0.78, 1]);

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  $: width = compact ? 68 : 84;
  $: height = compact ? 10 : 12;
  $: cellWidth = (width - gap * (segmentCount - 1)) / segmentCount;
  $: filled = clamped === 0 ? 0 : Math.max(1, Math.ceil((clamped / 100) * segmentCount));
  $: markerX = Math.max(1, Math.min(width - 1, (clamped / 100) * width));
  $: cells = range(segmentCount).map((index) => {
    const active = index < filled;
    const magnitude = ((index + 1) / segmentCount) * 100;
    const activeColor = colorScale(magnitude);

    return {
      index,
      active,
      x: index * (cellWidth + gap),
      width: cellWidth,
      fill: active ? activeColor : '#292d42',
      stroke: active ? activeColor : '#414866',
      opacity: active ? opacityScale(index) : 0.72
    };
  });
</script>

<span
  class="rank-bar"
  class:compact
  role="img"
  aria-label={`${ariaLabel}: ${Math.round(clamped)}%`}
  title={`${Math.round(clamped)}%`}
>
  <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" aria-hidden="true">
    {#each cells as cell}
      <rect
        class:active={cell.active}
        x={cell.x}
        y="0.5"
        width={cell.width}
        height={height - 1}
        fill={cell.fill}
        stroke={cell.stroke}
        opacity={cell.opacity}
      ></rect>
    {/each}

    {#if clamped > 0 && clamped < 100}
      <line class="rank-marker" x1={markerX} x2={markerX} y1="0" y2={height}></line>
    {/if}
  </svg>
</span>

<style>
  .rank-bar {
    display: inline-block;
    width: 84px;
    height: 12px;
    vertical-align: middle;
  }

  .rank-bar.compact {
    width: 68px;
    height: 10px;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    shape-rendering: crispEdges;
  }

  rect {
    vector-effect: non-scaling-stroke;
    transition: fill 140ms ease, stroke 140ms ease, opacity 140ms ease;
  }

  .rank-marker {
    stroke: #202438;
    stroke-width: 1;
    opacity: 0.8;
    vector-effect: non-scaling-stroke;
  }
</style>
