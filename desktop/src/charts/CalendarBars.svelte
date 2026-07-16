<script lang="ts">
  import { curveMonotoneX, line, scaleLinear } from 'd3';
  import type { TimelineGridDay } from '../types';

  export let days: TimelineGridDay[] = [];
  export let selected = '';
  export let ariaLabel = '';
  export let emptyLabel = '';
  export let barsLabel = '';
  export let averageLabel = '';
  export let detailLabel: (day: string, turns: number) => string = (day) => day;
  export let onSelect: (day: string) => void = () => {};

  const WIDTH = 920;
  const HEIGHT = 150;
  const LEFT = 36;
  const RIGHT = 8;
  const TOP = 12;
  const BOTTOM = 26;

  type Slot = { day: string; turns: number; inPeriod: boolean };

  function parse(day: string): Date {
    const [y, m, d] = day.split('-').map(Number);
    return new Date(y, m - 1, d);
  }

  function fmt(date: Date): string {
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }

  function weekday(day: string): string {
    return parse(day).toLocaleDateString(undefined, { weekday: 'short' });
  }

  // One slot per calendar day from the first shipped day through today, so
  // quiet stretches stay visible as gaps instead of compressing away.
  $: slots = (() => {
    if (!days.length) return [] as Slot[];
    const byDay = new Map(days.map((d) => [d.day, d]));
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const lastData = parse(days[days.length - 1].day);
    const end = lastData > today ? lastData : today;
    const out: Slot[] = [];
    const cursor = parse(days[0].day);
    while (cursor <= end) {
      const day = fmt(cursor);
      const data = byDay.get(day);
      out.push({ day, turns: data?.turns ?? 0, inPeriod: data?.in_period ?? true });
      cursor.setDate(cursor.getDate() + 1);
    }
    return out;
  })();

  $: max = Math.max(1, ...slots.map((slot) => slot.turns));
  $: plotWidth = WIDTH - LEFT - RIGHT;
  $: plotHeight = HEIGHT - TOP - BOTTOM;
  $: slotWidth = slots.length ? plotWidth / slots.length : plotWidth;
  $: barWidth = Math.max(1.5, Math.min(30, slotWidth * 0.72));
  $: y = scaleLinear().domain([0, max]).nice(3).range([TOP + plotHeight, TOP]);
  $: ticks = y.ticks(3).filter((tick) => tick > 0);
  $: labelStep = Math.max(1, Math.ceil(slots.length / 12));
  $: baseline = TOP + plotHeight;
  $: showAverage = slots.length >= 8;
  $: rolling = slots.map((_, index) => {
    const window = slots.slice(Math.max(0, index - 6), index + 1);
    return window.reduce((sum, slot) => sum + slot.turns, 0) / window.length;
  });
  $: averagePath = showAverage
    ? (line<number>()
        .x((_, index) => LEFT + slotWidth * index + slotWidth / 2)
        .y((value) => y(value))
        .curve(curveMonotoneX)(rolling) ?? '')
    : '';
</script>

<div class="calendar-bars" role="img" aria-label={ariaLabel}>
  {#if slots.length}
    <div class="chart-legend" aria-hidden="true">
      <span><i class="bar-key"></i>{barsLabel}</span>
      {#if showAverage}<span><i class="line-key"></i>{averageLabel}</span>{/if}
    </div>
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} preserveAspectRatio="none">
      {#each ticks as tick (tick)}
        <line class="grid" x1={LEFT} x2={WIDTH - RIGHT} y1={y(tick)} y2={y(tick)}></line>
        <text class="axis-label" x={LEFT - 6} y={y(tick) + 3} text-anchor="end">{tick}</text>
      {/each}
      <line class="baseline" x1={LEFT} x2={WIDTH - RIGHT} y1={baseline} y2={baseline}></line>
      {#each slots as slot, index (slot.day)}
        {#if slot.turns > 0}
          <rect
            class="bar"
            class:dim={!slot.inPeriod}
            class:selected={slot.day === selected}
            x={LEFT + slotWidth * index + (slotWidth - barWidth) / 2}
            y={y(slot.turns)}
            width={barWidth}
            height={Math.max(2, baseline - y(slot.turns))}
            role="button"
            tabindex="0"
            aria-label={`${weekday(slot.day)} ${detailLabel(slot.day, slot.turns)}`}
            aria-pressed={slot.day === selected}
            onclick={() => onSelect(slot.day)}
            onkeydown={(event) => (event.key === 'Enter' || event.key === ' ') && onSelect(slot.day)}
          >
            <title>{`${weekday(slot.day)} ${detailLabel(slot.day, slot.turns)}`}</title>
          </rect>
        {/if}
        {#if index % labelStep === 0 || index === slots.length - 1}
          <text
            class="axis-label"
            x={LEFT + slotWidth * index + slotWidth / 2}
            y={HEIGHT - 8}
            text-anchor={index === 0 ? 'start' : index === slots.length - 1 ? 'end' : 'middle'}
          >{slot.day.slice(5)}</text>
        {/if}
      {/each}
      {#if showAverage}
        <path class="average-line" d={averagePath}></path>
      {/if}
      {#each slots as slot, index (slot.day)}
        {#if slot.day === selected}
          <line
            class="selected-marker"
            x1={LEFT + slotWidth * index + slotWidth / 2}
            x2={LEFT + slotWidth * index + slotWidth / 2}
            y1={baseline}
            y2={baseline + 4}
          ></line>
        {/if}
      {/each}
    </svg>
  {:else}
    <div class="chart-empty">{emptyLabel}</div>
  {/if}
</div>

<style>
  .calendar-bars {
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
    background: var(--color-primary);
  }

  .line-key {
    height: 2px;
    background: var(--color-warning);
  }

  svg {
    display: block;
    width: 100%;
    height: 160px;
  }

  .grid {
    stroke: var(--chart-grid);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .baseline {
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
    fill: var(--color-primary);
    opacity: 0.72;
    cursor: pointer;
    transition: opacity var(--motion-fast) var(--ease-standard);
  }

  .bar.dim {
    opacity: 0.28;
  }

  .bar:hover,
  .bar.selected {
    opacity: 1;
  }

  .bar.selected {
    stroke: var(--color-on-surface);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .bar:focus-visible {
    outline: none;
    stroke: var(--color-secondary);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .average-line {
    fill: none;
    stroke: var(--color-warning);
    stroke-width: 1.5;
    opacity: 0.9;
    vector-effect: non-scaling-stroke;
    pointer-events: none;
  }

  .selected-marker {
    stroke: var(--color-on-surface);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
    pointer-events: none;
  }

  .chart-empty {
    min-height: 160px;
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
