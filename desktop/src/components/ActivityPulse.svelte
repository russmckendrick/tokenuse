<script lang="ts">
  import type { ActivityMetric } from '../types';

  export let points: ActivityMetric[] = [];
  export let density: 'compact' | 'roomy' = 'roomy';

  $: maxCalls = points.reduce((max, point) => Math.max(max, point.calls), 0);
  $: totalCalls = points.reduce((total, point) => total + point.calls, 0);
  $: high = points.length
    ? points.reduce((best, point) => (point.value > best.value ? point : best), points[0])
    : null;
  $: latest = points.length ? points[points.length - 1] : null;
  $: firstLabel = points[0]?.label ?? '-';
  $: lastLabel = latest?.label ?? '-';
  $: dense = points.length > 192;
  $: minBarWidth = dense ? 1 : 3;

  function heightFromValue(value: number) {
    const clamped = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
    return clamped === 0 ? '0%' : `${Math.max(12, clamped)}%`;
  }

  function heightFromCalls(calls: number) {
    if (maxCalls === 0) return '0%';
    return `${Math.max(12, Math.round((calls / maxCalls) * 100))}%`;
  }

  function compactCount(value: number) {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
    return value.toLocaleString();
  }
</script>

{#if points.length}
  <div class="activity-pulse" class:compact={density === 'compact'}>
    <div class="pulse-label spend">spend</div>
    <div class="pulse-track" class:dense style={`grid-template-columns: repeat(${points.length}, minmax(${minBarWidth}px, 1fr));`}>
      {#each points as point}
        <span
          class="pulse-bar spend-bar"
          class:peak={point === high}
          style={`height: ${heightFromValue(point.value)}`}
          title={`${point.label} · ${point.cost}`}
        ></span>
      {/each}
    </div>

    <div class="pulse-label calls">calls</div>
    <div class="pulse-track calls-track" class:dense style={`grid-template-columns: repeat(${points.length}, minmax(${minBarWidth}px, 1fr));`}>
      {#each points as point}
        <span
          class="pulse-bar calls-bar"
          style={`height: ${heightFromCalls(point.calls)}`}
          title={`${point.label} · ${point.calls.toLocaleString()} calls`}
        ></span>
      {/each}
    </div>

    <div class="pulse-meta">
      <span>range <strong>{firstLabel}</strong> to <strong>{lastLabel}</strong></span>
      <span>high <strong>{high?.label ?? '-'}</strong> <b>{high?.cost ?? '-'}</b></span>
      <span>latest <b>{latest?.cost ?? '-'}</b></span>
      <span>calls <strong>{compactCount(totalCalls)}</strong></span>
    </div>
  </div>
{:else}
  <div class="activity-empty">no activity in this view</div>
{/if}

<style>
  .activity-pulse {
    height: 100%;
    min-height: 108px;
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    grid-template-rows: minmax(28px, 1fr) minmax(28px, 1fr) auto;
    gap: 2px 8px;
    align-items: end;
  }

  .activity-pulse.compact {
    min-height: 86px;
  }

  .pulse-label {
    align-self: center;
    font-weight: 800;
  }

  .pulse-label.spend {
    color: #ff8f40;
  }

  .pulse-label.calls {
    color: #7ebcff;
  }

  .pulse-track {
    position: relative;
    min-width: 0;
    height: 100%;
    min-height: 30px;
    display: grid;
    gap: 2px;
    align-items: end;
    border-top: 2px solid #7ebcff;
  }

  .pulse-track.dense {
    gap: 1px;
  }

  .calls-track {
    border-top-style: dotted;
    border-bottom: 2px solid #7ebcff;
  }

  .pulse-bar {
    min-width: 2px;
    justify-self: stretch;
    align-self: end;
  }

  .spend-bar {
    background: #ff8f40;
  }

  .spend-bar.peak {
    background: #ff5f6d;
  }

  .calls-bar {
    background: #4df3e8;
  }

  .pulse-meta {
    grid-column: 1 / -1;
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
</style>
