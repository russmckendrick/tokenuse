<script lang="ts">
  import type { TimelineGridDay } from '../types';

  export let days: TimelineGridDay[] = [];
  export let selected = '';
  export let ariaLabel = '';
  export let lessLabel = '';
  export let moreLabel = '';
  export let detailLabel: (day: string, turns: number) => string = (day) => day;
  export let onSelect: (day: string) => void = () => {};

  type Cell = { day: string; turns: number; beyond: boolean };

  /* Always render a full GitHub-style year ending today, whatever the data
     span — sparse histories read as an emptier year, never a smaller grid. */
  const WEEKS = 53;

  function parse(day: string): Date {
    const [y, m, d] = day.split('-').map(Number);
    return new Date(y, m - 1, d);
  }

  function fmt(date: Date): string {
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }

  $: counts = new Map(days.map((d) => [d.day, d.turns]));
  $: maxTurns = days.reduce((max, d) => Math.max(max, d.turns), 0);

  // Column-major weeks, Monday-first, ending at today (or the newest data
  // day if it is somehow ahead of the clock); cells past the end pad the
  // final week invisibly.
  $: weeks = (() => {
    if (!days.length) return [] as Cell[][];
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const lastData = parse(days[days.length - 1].day);
    const end = lastData > today ? lastData : today;
    const start = new Date(end);
    start.setDate(start.getDate() - ((start.getDay() + 6) % 7) - (WEEKS - 1) * 7);
    const out: Cell[][] = [];
    const cursor = new Date(start);
    for (let w = 0; w < WEEKS; w += 1) {
      const week: Cell[] = [];
      for (let i = 0; i < 7; i += 1) {
        const day = fmt(cursor);
        week.push({ day, turns: counts.get(day) ?? 0, beyond: cursor > end });
        cursor.setDate(cursor.getDate() + 1);
      }
      out.push(week);
    }
    return out;
  })();

  $: weekdayLabels = weeks.length
    ? weeks[0].map((cell, idx) =>
        idx === 0 || idx === 2 || idx === 4
          ? parse(cell.day).toLocaleDateString(undefined, { weekday: 'short' })
          : ''
      )
    : [];

  $: monthLabels = weeks.map((week, idx) => {
    const month = parse(week[0].day).getMonth();
    const prev = idx > 0 ? parse(weeks[idx - 1][0].day).getMonth() : -1;
    return month !== prev
      ? parse(week[0].day).toLocaleDateString(undefined, { month: 'short' })
      : '';
  });

  /* Single-hue intensity ramp on the panel accent, GitHub-style. */
  function level(turns: number): number {
    if (maxTurns === 0 || turns === 0) return 0;
    const ratio = turns / maxTurns;
    if (ratio <= 0.25) return 1;
    if (ratio <= 0.5) return 2;
    if (ratio <= 0.75) return 3;
    return 4;
  }
</script>

{#if weeks.length}
  <div class="commit-grid" role="group" aria-label={ariaLabel}>
    <div class="months" aria-hidden="true">
      {#each monthLabels as label, idx (idx)}
        <span>{label}</span>
      {/each}
    </div>
    <div class="body">
      <div class="weekdays" aria-hidden="true">
        {#each weekdayLabels as label, idx (idx)}
          <span>{label}</span>
        {/each}
      </div>
      <div class="cells">
        {#each weeks as week, weekIdx (weekIdx)}
          {#each week as cell (cell.day)}
            {#if cell.beyond}
              <span class="cell spacer"></span>
            {:else if cell.turns > 0}
              <button
                type="button"
                class={`cell q${level(cell.turns)}`}
                class:selected={cell.day === selected}
                title={detailLabel(cell.day, cell.turns)}
                aria-label={detailLabel(cell.day, cell.turns)}
                aria-pressed={cell.day === selected}
                onclick={() => onSelect(cell.day)}
              ></button>
            {:else}
              <span class="cell" title={cell.day}></span>
            {/if}
          {/each}
        {/each}
      </div>
    </div>
    <div class="legend" aria-hidden="true">
      <span>{lessLabel}</span>
      <i class="cell q0"></i>
      <i class="cell q1"></i>
      <i class="cell q2"></i>
      <i class="cell q3"></i>
      <i class="cell q4"></i>
      <span>{moreLabel}</span>
    </div>
  </div>
{/if}

<style>
  .commit-grid {
    --cell: 13px;
    --cell-gap: 3px;
    --label-w: 30px;
    display: inline-flex;
    flex-direction: column;
    gap: var(--cell-gap);
    max-width: 100%;
    overflow-x: auto;
  }

  .months {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: calc(var(--cell) + var(--cell-gap));
    margin-left: calc(var(--label-w) + 8px);
    height: 14px;
  }

  .months span,
  .weekdays span,
  .legend span {
    font-family: var(--font-ui);
    font-size: 10px;
    color: var(--color-muted-2);
    white-space: nowrap;
  }

  .months span {
    overflow: visible;
    line-height: 14px;
  }

  .body {
    display: flex;
    gap: 8px;
  }

  .weekdays {
    display: grid;
    grid-template-rows: repeat(7, var(--cell));
    gap: var(--cell-gap);
    width: var(--label-w);
    flex: 0 0 auto;
  }

  .weekdays span {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    line-height: var(--cell);
  }

  .cells {
    display: grid;
    grid-auto-flow: column;
    grid-template-rows: repeat(7, var(--cell));
    grid-auto-columns: var(--cell);
    gap: var(--cell-gap);
  }

  .cell {
    width: var(--cell);
    height: var(--cell);
    border-radius: 3px;
    background: var(--color-bar-empty);
    border: none;
    padding: 0;
    display: block;
  }

  .cell.q1 {
    background: color-mix(in srgb, var(--color-primary) 28%, var(--color-neutral));
  }

  .cell.q2 {
    background: color-mix(in srgb, var(--color-primary) 52%, var(--color-neutral));
  }

  .cell.q3 {
    background: color-mix(in srgb, var(--color-primary) 76%, var(--color-neutral));
  }

  .cell.q4 {
    background: var(--color-primary);
  }

  button.cell {
    cursor: pointer;
    transition: filter var(--motion-fast) var(--ease-standard);
  }

  button.cell:hover {
    filter: brightness(1.25);
  }

  button.cell:focus-visible,
  .cell.selected {
    outline: 1px solid var(--color-on-surface);
    outline-offset: 1px;
  }

  .cell.spacer {
    visibility: hidden;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: var(--cell-gap);
    justify-content: flex-end;
    margin-top: var(--cell-gap);
  }

  .legend .cell {
    width: 10px;
    height: 10px;
    flex: 0 0 auto;
  }

  .legend span {
    margin: 0 4px;
  }

  @media (prefers-reduced-motion: reduce) {
    button.cell {
      transition: none;
    }
  }
</style>
