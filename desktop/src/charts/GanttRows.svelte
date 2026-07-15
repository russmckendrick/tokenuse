<script lang="ts">
  import { toolColor } from './colors';
  import type { TimelineSessionRow } from '../types';

  export let rows: TimelineSessionRow[] = [];
  export let windowStartMin = 0;
  export let windowEndMin = 24 * 60;
  export let ariaLabel = '';
  export let emptyLabel = '';
  export let turnsLabel = (count: number) => `${count}`;

  const rowHeight = 22;
  const barHeight = 10;
  const chartWidth = 640;

  $: start = Math.max(0, Math.min(windowStartMin, windowEndMin));
  $: end = Math.min(24 * 60, Math.max(windowEndMin, start + 30));
  $: span = Math.max(1, end - start);
  $: height = Math.max(rows.length, 1) * rowHeight;
  $: hourTicks = buildTicks(start, end);

  function buildTicks(from: number, to: number): number[] {
    const firstHour = Math.ceil(from / 60);
    const lastHour = Math.floor(to / 60);
    const step = lastHour - firstHour > 12 ? 3 : lastHour - firstHour > 6 ? 2 : 1;
    const ticks: number[] = [];
    for (let h = firstHour; h <= lastHour; h += step) {
      ticks.push(h * 60);
    }
    return ticks;
  }

  function x(minute: number): number {
    return ((minute - start) / span) * chartWidth;
  }

  function fmtMinute(minute: number): string {
    const h = Math.floor(minute / 60);
    const m = minute % 60;
    return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`;
  }
</script>

{#if rows.length === 0}
  <p class="gantt-empty">{emptyLabel}</p>
{:else}
  <div class="gantt" role="img" aria-label={ariaLabel}>
    <div class="gantt-labels" style={`--row-height: ${rowHeight}px`}>
      {#each rows as row (row.session_key)}
        <div class="gantt-label" title={row.project}>
          <span class="gantt-project">{row.project}</span>
          <span class="gantt-meta mono">{row.tool_label} · {turnsLabel(row.turns)} · {row.cost}</span>
        </div>
      {/each}
    </div>
    <svg
      viewBox={`0 0 ${chartWidth} ${height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      {#each hourTicks as tick}
        <line class="gantt-tick" x1={x(tick)} x2={x(tick)} y1="0" y2={height}></line>
      {/each}
      {#each rows as row, i (row.session_key)}
        {#each row.blocks as block}
          <rect
            x={x(block.start_min)}
            y={i * rowHeight + (rowHeight - barHeight) / 2}
            width={Math.max(2, x(block.end_min) - x(block.start_min))}
            height={barHeight}
            fill={toolColor(row.tool, i)}
            opacity="0.9"
          >
            <title>{`${row.project} ${fmtMinute(block.start_min)}–${fmtMinute(block.end_min)}`}</title>
          </rect>
        {/each}
      {/each}
    </svg>
  </div>
  <div class="gantt-axis mono" aria-hidden="true">
    <span>{fmtMinute(start)}</span>
    <span>{fmtMinute(end)}</span>
  </div>
{/if}

<style>
  .gantt {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 8px;
    align-items: stretch;
  }

  .gantt-labels {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .gantt-label {
    height: var(--row-height);
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    border-bottom: 1px solid var(--color-border-row);
  }

  .gantt-label:last-child {
    border-bottom: none;
  }

  .gantt-project {
    font-size: 11px;
    color: var(--color-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .gantt-meta {
    font-size: 9px;
    color: var(--color-muted-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  .gantt-tick {
    stroke: var(--color-border-row);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .gantt-axis {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
    padding-left: 188px;
    font-size: 9px;
    color: var(--color-muted-2);
  }

  .gantt-empty {
    color: var(--color-muted);
    font-size: 12px;
    margin: 0;
  }
</style>
