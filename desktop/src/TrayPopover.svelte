<script lang="ts">
  import { onMount } from 'svelte';
  import { ExternalLink, RefreshCw, X } from 'lucide-svelte';
  import { api } from './api';
  import type { ActivityMetric, ToolLimitSection, TraySnapshot } from './types';

  let snapshot: TraySnapshot | null = null;
  let error: string | null = null;
  let busy = false;
  let pollTimer: number | undefined;

  onMount(() => {
    void load();
    pollTimer = window.setInterval(() => void loadSilent(), 3000);
    window.addEventListener('keydown', handleKey);

    return () => {
      if (pollTimer !== undefined) {
        window.clearInterval(pollTimer);
      }
      window.removeEventListener('keydown', handleKey);
    };
  });

  async function load() {
    busy = true;
    error = null;
    try {
      snapshot = await api.traySnapshot();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function loadSilent() {
    try {
      snapshot = await api.traySnapshot();
    } catch {
      // Keep the last good popover render during transient IPC errors.
    }
  }

  function handleKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      void api.hideTrayPopover();
    }
  }

  async function openFullApp() {
    await api.openMainWindow();
  }

  async function closePopover() {
    await api.hideTrayPopover();
  }

  function count(value: number) {
    return value.toLocaleString();
  }

  function usageSections(): ToolLimitSection[] {
    return snapshot?.usage.sections ?? [];
  }

  function activityPoints(): ActivityMetric[] {
    return snapshot?.dashboard.activity_timeline ?? [];
  }

  function peakPoint() {
    const points = activityPoints();
    return points.reduce<ActivityMetric | null>((best, point) => {
      if (!best || point.value > best.value) return point;
      return best;
    }, null);
  }

  function latestPoint() {
    const points = activityPoints();
    return points.length ? points[points.length - 1] : null;
  }

  function totalCalls() {
    return activityPoints().reduce((total, point) => total + point.calls, 0);
  }

  function bucketHeight(value: number) {
    const clamped = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
    return clamped === 0 ? '2px' : `${Math.max(8, clamped)}%`;
  }
</script>

<div class="tray-popover" class:is-busy={busy}>
  <div class="popover-head">
    <div class="brand-lockup">
      <svg class="brand-bars" viewBox="0 0 440 560" aria-hidden="true">
        <defs>
          <linearGradient id="tray-brand-bar-gradient" x1="0" y1="0" x2="0" y2="560" gradientUnits="userSpaceOnUse">
            <stop offset="0%" stop-color="#ffc06a" />
            <stop offset="45%" stop-color="#ff9a4d" />
            <stop offset="100%" stop-color="#f26a3d" />
          </linearGradient>
        </defs>
        <rect x="0" y="280" width="80" height="280" rx="16" fill="url(#tray-brand-bar-gradient)" />
        <rect x="120" y="160" width="80" height="400" rx="16" fill="url(#tray-brand-bar-gradient)" />
        <rect x="240" y="0" width="80" height="560" rx="16" fill="url(#tray-brand-bar-gradient)" />
        <rect x="360" y="120" width="80" height="440" rx="16" fill="url(#tray-brand-bar-gradient)" />
      </svg>
      <div>
        <strong>Token Use</strong>
        <span>24 Hours</span>
      </div>
    </div>
    <button class="close-button" type="button" title="Close" onclick={closePopover}>
      <X size={15} />
    </button>
  </div>

  {#if snapshot}
    <div class="metric-grid" aria-label="24 hour summary">
      <div>
        <span>cost</span>
        <strong>{snapshot.dashboard.summary.cost}</strong>
        <small>{snapshot.currency}</small>
      </div>
      <div>
        <span>calls</span>
        <strong>{snapshot.dashboard.summary.calls}</strong>
        <small>{snapshot.dashboard.summary.sessions} sessions</small>
      </div>
      <div>
        <span>tokens</span>
        <strong>{snapshot.dashboard.summary.input}</strong>
        <small>{snapshot.dashboard.summary.output} out</small>
      </div>
      <div>
        <span>cache</span>
        <strong>{snapshot.dashboard.summary.cache_hit}</strong>
        <small>{snapshot.dashboard.summary.cached}</small>
      </div>
    </div>

    {#if error || snapshot.status}
      <div class:error={Boolean(error)} class="popover-status">{error ?? snapshot.status}</div>
    {/if}

    <div class="activity-card">
      <div class="card-head">
        <span>Activity</span>
        <strong>{latestPoint()?.cost ?? '-'}</strong>
      </div>
      <div class="sparkline" aria-hidden="true">
        {#each activityPoints() as point}
          <i class:peak={point === peakPoint()} style={`height: ${bucketHeight(point.value)}`}></i>
        {/each}
      </div>
      <div class="activity-meta">
        <span>high {peakPoint()?.label ?? '-'}</span>
        <span>{count(totalCalls())} calls</span>
      </div>
    </div>

    <div class="tool-list" aria-label="Tool usage">
      {#each usageSections() as section}
        <div class="tool-row">
          <div class="tool-title">
            <strong>{section.tool}</strong>
            <span>{section.usage.cost}</span>
          </div>
          <div class="tool-buckets" aria-hidden="true">
            {#each section.usage.buckets as bucket}
              <i style={`height: ${bucketHeight(bucket)}`}></i>
            {/each}
          </div>
          <div class="tool-meta">
            <span>{count(section.usage.calls)} calls</span>
            <span>{section.usage.tokens}</span>
            <span>{section.usage.last_seen}</span>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="tray-loading">Token Use</div>
  {/if}

  <div class="popover-actions">
    <button class="secondary-action" type="button" title="Refresh" onclick={load}>
      <RefreshCw size={15} />
      Refresh
    </button>
    <button class="primary-action" type="button" onclick={openFullApp}>
      <ExternalLink size={15} />
      Open
    </button>
  </div>
</div>

<style>
  .tray-popover {
    width: 100vw;
    height: 100vh;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 10px;
    color: #cbd4f2;
    background: #202438;
    border: 1px solid #414866;
    overflow: hidden;
  }

  .tray-popover.is-busy {
    cursor: progress;
  }

  .popover-head,
  .brand-lockup,
  .card-head,
  .activity-meta,
  .tool-title,
  .tool-meta,
  .popover-actions {
    min-width: 0;
    display: flex;
    align-items: center;
  }

  .popover-head,
  .card-head,
  .tool-title,
  .popover-actions {
    justify-content: space-between;
    gap: 8px;
  }

  .popover-head {
    min-height: 38px;
    padding-bottom: 7px;
    border-bottom: 1px solid #ff8f40;
  }

  .brand-lockup {
    gap: 8px;
  }

  .brand-lockup > div {
    min-width: 0;
    display: grid;
    gap: 1px;
  }

  .brand-bars {
    width: 20px;
    height: 26px;
    flex: 0 0 auto;
  }

  strong,
  span,
  small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .popover-head strong {
    color: #ff8f40;
    font-size: 15px;
    font-weight: 800;
  }

  .popover-head span,
  small,
  .activity-meta,
  .tool-meta,
  .popover-status {
    color: #a1a7c3;
  }

  .close-button {
    width: 28px;
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    color: #a1a7c3;
    background: #25293d;
  }

  .metric-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .metric-grid div {
    min-width: 0;
    min-height: 56px;
    display: grid;
    align-content: center;
    gap: 1px;
    padding: 6px 7px;
    border: 1px solid #ff8f40;
    border-radius: 3px;
    background: #202438;
  }

  .metric-grid span {
    color: #a1a7c3;
    font-size: 10px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .metric-grid strong {
    color: #ffd60a;
    font-size: 18px;
    line-height: 1.08;
  }

  .popover-status {
    min-height: 24px;
    padding: 4px 7px;
    border: 1px solid #414866;
    border-radius: 3px;
  }

  .popover-status.error {
    color: #ff5f6d;
    border-color: #ff5f6d;
  }

  .activity-card,
  .tool-row {
    min-width: 0;
    display: grid;
    gap: 5px;
    border: 1px solid #414866;
    border-radius: 3px;
    background: #25293d;
  }

  .activity-card {
    min-height: 86px;
    padding: 7px;
    border-color: #4df3e8;
  }

  .card-head span {
    color: #4df3e8;
    font-weight: 800;
  }

  .card-head strong,
  .tool-title span {
    color: #ffd60a;
    font-weight: 800;
  }

  .sparkline,
  .tool-buckets {
    min-width: 0;
    display: grid;
    align-items: end;
    gap: 1px;
    border-bottom: 1px solid #4df3e8;
  }

  .sparkline {
    height: 36px;
    grid-template-columns: repeat(24, minmax(3px, 1fr));
  }

  .sparkline i,
  .tool-buckets i {
    min-height: 2px;
    background: #4df3e8;
  }

  .sparkline i.peak {
    background: #ff5f6d;
  }

  .activity-meta,
  .tool-meta {
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
  }

  .tool-list {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    gap: 5px;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .tool-row {
    min-height: 58px;
    padding: 6px;
  }

  .tool-title strong {
    color: #cbd4f2;
    font-size: 13px;
  }

  .tool-buckets {
    height: 18px;
    grid-template-columns: repeat(24, minmax(2px, 1fr));
  }

  .popover-actions {
    min-height: 37px;
    padding-top: 7px;
    border-top: 1px solid #414866;
  }

  .secondary-action,
  .primary-action {
    min-height: 30px;
  }

  .primary-action {
    min-width: 112px;
    color: #202438;
    background: #ff8f40;
    border-color: #ff8f40;
    font-weight: 800;
  }

  .tray-loading {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    place-items: center;
    color: #ff8f40;
    font-weight: 800;
  }
</style>
