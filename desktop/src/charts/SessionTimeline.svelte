<script lang="ts">
  import { toolColor } from './colors';
  import type { TimelineSessionRow } from '../types';

  export let rows: TimelineSessionRow[] = [];
  export let windowStartMin = 0;
  export let windowEndMin = 24 * 60;
  export let headers: { time: string; project: string; tool: string; turns: string; cost: string } = {
    time: '',
    project: '',
    tool: '',
    turns: '',
    cost: ''
  };
  export let ariaLabel = '';
  export let emptyLabel = '';
  export let selectedKey = '';
  export let onSelect: (key: string) => void = () => {};

  /* Axis padded to whole hours so gridlines land on readable times. */
  $: axisStart = Math.max(0, Math.floor(Math.min(windowStartMin, windowEndMin) / 60) * 60);
  $: axisEnd = Math.min(24 * 60, Math.max(Math.ceil(windowEndMin / 60) * 60, axisStart + 60));
  $: span = Math.max(1, axisEnd - axisStart);
  $: ticks = buildTicks(axisStart, axisEnd);

  function buildTicks(from: number, to: number): number[] {
    const firstHour = Math.ceil(from / 60);
    const lastHour = Math.floor(to / 60);
    const step = lastHour - firstHour > 12 ? 3 : lastHour - firstHour > 6 ? 2 : 1;
    const out: number[] = [];
    for (let h = firstHour; h <= lastHour; h += step) {
      out.push(h * 60);
    }
    return out;
  }

  function pct(minute: number): number {
    return ((minute - axisStart) / span) * 100;
  }

  function fmtMinute(minute: number): string {
    const h = Math.floor(minute / 60);
    const m = minute % 60;
    return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`;
  }

  function rowRange(row: TimelineSessionRow): string {
    const first = row.blocks[0];
    const last = row.blocks[row.blocks.length - 1];
    if (!first || !last) return '';
    return `${fmtMinute(first.start_min)}–${fmtMinute(last.end_min)}`;
  }
</script>

{#if rows.length === 0}
  <p class="session-empty">{emptyLabel}</p>
{:else}
  <div class="session-scroll" role="group" aria-label={ariaLabel}>
    <div class="session-table">
      <div class="session-head session-grid" aria-hidden="true">
        <span>{headers.time}</span>
        <span>{headers.project}</span>
        <span>{headers.tool}</span>
        <span class="num">{headers.turns}</span>
        <span class="num">{headers.cost}</span>
        <span class="head-track">
          {#each ticks as tick (tick)}
            <i
              class="tick-label mono"
              class:edge-start={pct(tick) < 4}
              class:edge-end={pct(tick) > 96}
              style={`left: ${pct(tick)}%`}
            >{fmtMinute(tick)}</i>
          {/each}
        </span>
      </div>
      {#each rows as row, i (row.session_key)}
        <button
          type="button"
          class="session-row session-grid"
          class:selected={row.session_key === selectedKey}
          title={row.project}
          aria-pressed={row.session_key === selectedKey}
          onclick={() => onSelect(row.session_key)}
        >
          <span class="mono time-range">{rowRange(row)}</span>
          <span class="project">{row.project}</span>
          <span class="tool"><i class="tool-dot" style={`background: ${toolColor(row.tool, i)}`}></i>{row.tool_label}</span>
          <span class="mono num">{row.turns}</span>
          <span class="mono num">{row.cost}</span>
          <span class="track">
            {#each ticks as tick (tick)}
              <i class="tick-line" style={`left: ${pct(tick)}%`}></i>
            {/each}
            {#each row.blocks as block (`${block.start_min}-${block.end_min}`)}
              <i
                class="block"
                style={`left: ${pct(block.start_min)}%; width: ${Math.max(0, pct(block.end_min) - pct(block.start_min))}%; background: ${toolColor(row.tool, i)}`}
                title={`${fmtMinute(block.start_min)}–${fmtMinute(block.end_min)}`}
              ></i>
            {/each}
          </span>
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .session-scroll {
    max-height: 328px;
    overflow-y: auto;
    overflow-x: auto;
  }

  .session-table {
    min-width: 720px;
  }

  .session-grid {
    display: grid;
    grid-template-columns: 88px minmax(140px, 180px) 96px 48px 76px minmax(240px, 1fr);
    column-gap: 10px;
    align-items: center;
    width: 100%;
    border-left: 2px solid transparent;
    padding: 0 6px 0 4px;
  }

  .session-head {
    position: sticky;
    top: 0;
    z-index: 2;
    height: 28px;
    background: var(--color-neutral);
    border-bottom: 1px solid var(--color-border);
  }

  .session-head span {
    font-family: var(--font-ui);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Needs the extra specificity to beat `.session-head span`'s ellipsis
     clipping, or edge tick labels get cut. */
  .session-head span.head-track {
    position: relative;
    height: 100%;
    overflow: visible;
  }

  .tick-label {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    font-size: 9px;
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    color: var(--color-muted-2);
  }

  .tick-label.edge-start {
    transform: translate(0, -50%);
  }

  .tick-label.edge-end {
    transform: translate(-100%, -50%);
  }

  .session-row {
    height: 30px;
    background: transparent;
    border-top: 0;
    border-right: 0;
    border-bottom: 1px solid var(--color-border-row);
    text-align: left;
    cursor: pointer;
    transition: background var(--motion-fast) var(--ease-standard);
  }

  .session-row:last-child {
    border-bottom: none;
  }

  .session-row:hover {
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
  }

  .session-row.selected {
    border-left-color: var(--color-primary);
    background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  }

  .session-row:focus-visible {
    outline: 1px solid var(--color-secondary);
    outline-offset: -1px;
  }

  .session-row > span {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .time-range {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
  }

  .project {
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 600;
    color: var(--color-on-surface);
  }

  .tool {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-ui);
    font-size: 10px;
    color: var(--color-muted-2);
  }

  .tool-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: 0 0 auto;
  }

  .num {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .session-row .num {
    color: var(--color-on-surface);
  }

  .track {
    position: relative;
    height: 100%;
    overflow: hidden;
  }

  .tick-line {
    position: absolute;
    top: 0;
    bottom: 0;
    border-left: 1px solid var(--color-border-row);
  }

  .block {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    height: 12px;
    min-width: 3px;
    border-radius: 2px;
    opacity: 0.75;
    transition: opacity var(--motion-fast) var(--ease-standard);
  }

  .session-row:hover .block,
  .session-row.selected .block {
    opacity: 1;
  }

  .session-empty {
    color: var(--color-muted);
    font-size: 12px;
    margin: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .session-row {
      transition: none;
    }
  }
</style>
