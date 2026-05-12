<script lang="ts">
  import {
    AlertTriangle,
    BarChart3,
    ChevronRight,
    CircleDot,
    Database,
    ListChecks,
    ShieldCheck
  } from 'lucide-svelte';
  import { countUp, fadeIn } from '../../motion';
  import type { DesktopSnapshot } from '../../types';
  import {
    barStyle,
    baselineLabel,
    normalizeSeverity,
    runDetail,
    runLabel,
    type InsightModel,
    type InsightRow,
    type MainInsightsScreen
  } from './model';

  export let snapshot: DesktopSnapshot;
  export let model: InsightModel;
  export let openDetail: (row: InsightRow, from: MainInsightsScreen) => void;
  export let showScreen: (screen: MainInsightsScreen) => void;
</script>

<section class="screen overview-screen" use:fadeIn>
  <div class="overview-hero">
    <section class="digest-panel">
      <span class="eyebrow">{model.copy.overview_digest_title}</span>
      {#if snapshot.advice_running}
        <div class="run-skeleton" aria-live="polite">
          <span></span>
          <span></span>
          <span></span>
        </div>
      {:else if model.latestRun}
        <h3>{runLabel(model.latestRun, model.copy, model.latestRun)}</h3>
        <p>{runDetail(model.latestRun, model.copy)}</p>
      {:else}
        <h3>{model.copy.advice_empty}</h3>
        <p>{model.copy.overview_digest_empty}</p>
      {/if}
      <div class="context-badges">
        <span><Database size={13} /> {model.copy.local_badge}</span>
        <span><ShieldCheck size={13} /> {model.copy.scope_badge}</span>
        <span>{baselineLabel(snapshot.insights.baseline_window_days, model.copy)}</span>
      </div>
    </section>

    <dl class="kpi-strip">
      <div class="amount">
        <dt>{model.copy.kpi_savings}</dt>
        <dd use:countUp={snapshot.insights.summary.total_est_savings}>{snapshot.insights.summary.total_est_savings}</dd>
      </div>
      <div class="risk">
        <dt>{model.copy.kpi_risks}</dt>
        <dd use:countUp={String(model.severityCount('risk'))}>{model.severityCount('risk')}</dd>
      </div>
      <div class="warn">
        <dt>{model.copy.kpi_warns}</dt>
        <dd use:countUp={String(model.severityCount('warn'))}>{model.severityCount('warn')}</dd>
      </div>
      <div class="info">
        <dt>{model.copy.kpi_infos}</dt>
        <dd use:countUp={String(model.severityCount('info'))}>{model.severityCount('info')}</dd>
      </div>
    </dl>
  </div>

  <div class="overview-grid">
    <section class="panel top-actions">
      <div class="panel-head">
        <div>
          <span class="panel-kicker"><ListChecks size={14} /> {model.copy.top_actions_title}</span>
          <p>{model.copy.top_actions_detail}</p>
        </div>
        <button type="button" onclick={() => showScreen('actions')}>
          {model.copy.screen_actions}
          <ChevronRight size={14} />
        </button>
      </div>

      {#if model.topRows.length}
        <div class="top-action-list">
          {#each model.topRows as row (row.id)}
            <button
              type="button"
              class={`summary-row severity-${row.severity}`}
              onclick={() => openDetail(row, 'overview')}
            >
              <span class="row-rail"></span>
              <span class="row-body">
                <span class="row-meta">
                  <strong class={`severity-label ${row.severity}`}>{row.severityLabel}</strong>
                  <span>{row.sourceLabel}</span>
                  <span>{row.category}</span>
                  {#if row.confidence}<span>{row.confidence}</span>{/if}
                </span>
                <strong>{row.title}</strong>
                <span>{row.impact ?? row.body}</span>
              </span>
              {#if row.savings}<span class="money-chip">{row.savings}</span>{/if}
              <ChevronRight size={15} />
            </button>
          {/each}
        </div>
      {:else}
        <div class="empty-state">
          <CircleDot size={18} />
          <strong>{model.copy.actions_empty_title}</strong>
          <p>{model.copy.actions_empty_detail}</p>
        </div>
      {/if}
    </section>

    <aside class="overview-side">
      <section class="panel signal-digest">
        <div class="panel-head compact">
          <span class="panel-kicker"><BarChart3 size={14} /> {model.copy.signal_map_title}</span>
          <button type="button" onclick={() => showScreen('signals')}>
            {model.copy.screen_signals}
            <ChevronRight size={14} />
          </button>
        </div>
        <div class="bar-list">
          {#each snapshot.insights.summary.by_severity as severity}
            <div class={`bar-row severity-${normalizeSeverity(severity.id)}`} style={barStyle(severity.count, model.maxSeverityCount)}>
              <span>{severity.label}</span>
              <strong>{severity.count}</strong>
              <i></i>
            </div>
          {/each}
        </div>
      </section>

      <section class="panel latest-issue">
        <div class="panel-head compact">
          <span class="panel-kicker"><AlertTriangle size={14} /> {model.copy.latest_issue_title}</span>
          <button type="button" onclick={() => showScreen('runs')}>
            {model.copy.screen_runs}
            <ChevronRight size={14} />
          </button>
        </div>
        {#if model.latestFailure}
          <div class="run-note failed">
            <strong>{runLabel(model.latestFailure, model.copy, model.latestRun)}</strong>
            <p>{runDetail(model.latestFailure, model.copy)}</p>
          </div>
        {:else}
          <div class="run-note">
            <strong>{model.copy.no_failed_runs_title}</strong>
            <p>{model.copy.no_failed_runs_detail}</p>
          </div>
        {/if}
      </section>
    </aside>
  </div>
</section>
