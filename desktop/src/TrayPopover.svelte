<script lang="ts">
  import { onMount } from 'svelte';
  import { ExternalLink, RefreshCw, X } from 'lucide-svelte';
  import { api } from './api';
  import GaugeBar from './components/GaugeBar.svelte';
  import { reveal, staggeredReveal } from './motion';
  import type { LimitMetric, TraySnapshot } from './types';

  type UtilisationRow = { tool: string; limit: LimitMetric };

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

  function utilisationRows(snapshotValue: TraySnapshot): UtilisationRow[] {
    const rows: UtilisationRow[] = [];
    for (const section of snapshotValue.usage.sections) {
      for (const limit of section.limits) {
        if (!limit.stale) rows.push({ tool: section.tool, limit });
      }
    }
    return rows
      .sort((a, b) => b.limit.used - a.limit.used || a.tool.localeCompare(b.tool))
      .slice(0, 4);
  }

  $: utilisation = snapshot ? utilisationRows(snapshot) : [];
</script>

<div class="tray-popover" class:is-busy={busy} use:reveal={{ y: 4 }}>
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
        <strong>{snapshot?.copy.brand.name ?? ''}</strong>
        <span>{snapshot?.copy.tray.hours_24 ?? ''}</span>
      </div>
    </div>
    <button class="close-button" type="button" title={snapshot?.copy.actions.close ?? ''} onclick={closePopover}>
      <X size={15} />
    </button>
  </div>

  {#if snapshot}
    <div class="metric-grid" aria-label={snapshot.copy.tray.summary_aria} use:staggeredReveal={{ selector: ':scope > div', y: 3, stagger: 0.02 }}>
      <div>
        <span>{snapshot.copy.metrics.cost}</span>
        <strong>{snapshot.dashboard.summary.cost}</strong>
        <small>{snapshot.currency}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.calls}</span>
        <strong>{snapshot.dashboard.summary.calls}</strong>
        <small>{snapshot.dashboard.summary.sessions} {snapshot.copy.metrics.sessions}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.tokens}</span>
        <strong>{snapshot.dashboard.summary.input}</strong>
        <small>{snapshot.dashboard.summary.output} {snapshot.copy.metrics.out}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.cache}</span>
        <strong>{snapshot.dashboard.summary.cache_hit}</strong>
        <small>{snapshot.dashboard.summary.cached}</small>
      </div>
    </div>

    <div class="utilisation-card">
      <div class="card-head">
        <span>{snapshot.copy.desktop.utilisation}</span>
        <strong>{utilisation[0]?.limit.left ?? '-'}</strong>
      </div>
      <div class="limit-list" aria-label={snapshot.copy.desktop.utilisation} use:staggeredReveal={{ selector: ':scope > div', y: 3, stagger: 0.02 }}>
        {#each utilisation as row}
          <div class="limit-row">
            <div class="limit-head">
              <strong>{row.tool}</strong>
              <span>{row.limit.left}</span>
            </div>
            <GaugeBar used={row.limit.used} ariaLabel={`${row.tool} ${row.limit.scope}`} />
            <div class="limit-meta">
              <span>{row.limit.scope} · {row.limit.window}</span>
              <small>{snapshot.copy.tables.reset} {row.limit.reset}</small>
            </div>
          </div>
        {:else}
          <div class="empty-limits">{snapshot.copy.empty.no_data}</div>
        {/each}
      </div>
    </div>
  {:else}
    <div class="tray-loading" aria-busy="true" use:reveal></div>
  {/if}

  <div class="popover-actions">
    <button class="secondary-action" type="button" title={snapshot?.copy.actions.refresh ?? ''} onclick={load}>
      <RefreshCw size={15} />
      {snapshot?.copy.actions.refresh ?? ''}
    </button>
    <button class="primary-action" type="button" onclick={openFullApp}>
      <ExternalLink size={15} />
      {snapshot?.copy.actions.open ?? ''}
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
  .popover-actions {
    min-width: 0;
    display: flex;
    align-items: center;
  }

  .popover-head,
  .card-head,
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
  small {
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
    gap: 4px;
  }

  .metric-grid div {
    min-width: 0;
    min-height: 44px;
    display: grid;
    align-content: center;
    gap: 1px;
    padding: 4px 6px;
    border: 1px solid #ff8f40;
    border-radius: 3px;
    background: #202438;
  }

  .metric-grid span {
    color: #a1a7c3;
    font-size: 9px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .metric-grid strong {
    color: #ffd60a;
    font-size: 16px;
    line-height: 1.08;
  }

  .metric-grid small {
    font-size: 10px;
    line-height: 1.1;
  }

  .utilisation-card {
    min-width: 0;
    display: grid;
    gap: 5px;
    border: 1px solid #414866;
    border-radius: 3px;
    background: #25293d;
  }

  .card-head span {
    color: #4df3e8;
    font-weight: 800;
  }

  .card-head strong,
  .limit-head span {
    color: #ffd60a;
    font-weight: 800;
  }

  .utilisation-card {
    flex: 1 1 auto;
    min-height: 0;
    align-content: start;
    padding: 7px;
    overflow: hidden;
  }

  .limit-list {
    min-height: 0;
    display: grid;
    align-content: start;
    overflow: auto;
    scrollbar-width: thin;
  }

  .limit-row {
    min-width: 0;
    display: grid;
    gap: 3px;
    padding: 5px 1px;
    border-bottom: 1px solid var(--color-border-soft);
  }

  .limit-row:last-child {
    border-bottom: 0;
  }

  .limit-head,
  .limit-meta {
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .limit-head strong {
    color: var(--color-on-surface);
    font-size: 11.5px;
    line-height: 1.2;
  }

  .limit-head span {
    font-size: 11.5px;
    line-height: 1.1;
  }

  .limit-meta {
    color: var(--color-muted);
    font-size: 10px;
  }

  .limit-meta span,
  .limit-meta small {
    font-size: 10px;
  }

  .limit-list .empty-limits {
    min-height: 80px;
    display: grid;
    place-items: center;
    color: var(--color-muted);
    text-align: center;
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
