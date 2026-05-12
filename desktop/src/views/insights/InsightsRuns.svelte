<script lang="ts">
  import { CircleDot, Database, FileText } from 'lucide-svelte';
  import { fadeIn } from '../../motion';
  import type { AdviceDataScopeId, DesktopSnapshot } from '../../types';
  import AdviceGenerateControl from './AdviceGenerateControl.svelte';
  import {
    archiveSourceLabel,
    formatRunDate,
    generatedAtLabel,
    baselineLabel,
    runDetail,
    runLabel,
    runScopeLabel,
    runStatusLabel,
    type InsightModel
  } from './model';

  export let snapshot: DesktopSnapshot;
  export let model: InsightModel;
  export let selectedAdviceScope: AdviceDataScopeId;
  export let setAdviceScope: (scope: AdviceDataScopeId) => void;
  export let runSelectedAdvice: () => void;
</script>

<section class="screen runs-screen" use:fadeIn>
  <div class="screen-head">
    <div>
      <span class="eyebrow">{model.copy.screen_runs}</span>
      <h3>{model.copy.runs_title}</h3>
      <p>{model.copy.runs_subtitle}</p>
    </div>
  </div>

  <div class="runs-layout">
    <section class="panel run-list-panel">
      <div class="panel-head compact">
        <span class="panel-kicker"><FileText size={14} /> {model.copy.run_history_title}</span>
        <strong class="count-value">{snapshot.advice.runs.length}</strong>
      </div>

      {#if snapshot.advice.runs.length}
        <div class="run-list">
          {#each snapshot.advice.runs as run (run.id)}
            <article class:failed={run.status === 'failed'} class="run-row">
              <div>
                <span class={`status-chip status-${run.status}`}>{runStatusLabel(run.status, model.copy)}</span>
                <strong>{runLabel(run, model.copy, model.latestRun)}</strong>
                <p>{runDetail(run, model.copy)}</p>
              </div>
              <dl>
                <div>
                  <dt>{model.copy.generated_at_label}</dt>
                  <dd>{formatRunDate(run.created_at)}</dd>
                </div>
                <div>
                  <dt>{model.copy.detail_scope}</dt>
                  <dd>{runScopeLabel(run.data_scope, model.copy)}</dd>
                </div>
                <div>
                  <dt>{model.copy.source_advice}</dt>
                  <dd>{run.items.length}</dd>
                </div>
              </dl>
            </article>
          {/each}
        </div>
      {:else}
        <div class="empty-state">
          <CircleDot size={18} />
          <strong>{model.copy.advice_empty}</strong>
          <p>{model.copy.overview_digest_empty}</p>
        </div>
      {/if}
    </section>

    <aside class="runs-side">
      <section class="panel generate-panel">
        <AdviceGenerateControl
          copy={model.copy}
          adviceRunning={snapshot.advice_running}
          {selectedAdviceScope}
          {setAdviceScope}
          {runSelectedAdvice}
          controlId="runs"
        />
      </section>

      <section class="panel data-context-panel">
        <div class="panel-head compact">
          <span class="panel-kicker"><Database size={14} /> {model.copy.data_context_title}</span>
        </div>
        <dl class="data-context">
          <div>
            <dt>{model.copy.generated_at_label}</dt>
            <dd>{generatedAtLabel(snapshot.insights.generated_at, model.copy)}</dd>
          </div>
          <div>
            <dt>{model.copy.baseline_window_label}</dt>
            <dd>{baselineLabel(snapshot.insights.baseline_window_days, model.copy)}</dd>
          </div>
          <div>
            <dt>{model.copy.archive_source_label}</dt>
            <dd>{archiveSourceLabel(snapshot)}</dd>
          </div>
          <div>
            <dt>{model.copy.advice_tool_label}</dt>
            <dd>{model.adviceToolLabel}</dd>
          </div>
        </dl>
      </section>
    </aside>
  </div>
</section>
