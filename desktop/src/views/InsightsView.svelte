<script lang="ts">
  import { Check, Lightbulb, RotateCcw, Sparkles, X } from 'lucide-svelte';
  import RecommendationCard from '../components/RecommendationCard.svelte';
  import { staggeredReveal } from '../motion';
  import type {
    AdviceDataScopeId,
    AdviceItemStatusId,
    AdviceItemView,
    AdviceRunView,
    DesktopSnapshot
  } from '../types';

  export let snapshot: DesktopSnapshot;
  export let generateAdvice: (scope: AdviceDataScopeId) => void;
  export let updateAdviceItemStatus: (itemId: number, status: AdviceItemStatusId) => void;

  let scopeOpen = false;
  $: view = snapshot.insights;
  $: latestRun = snapshot.advice.runs[0] ?? null;
  $: severityCount = (id: string) =>
    view.summary.by_severity.find((s) => s.id === id)?.count ?? 0;

  function copyTemplate(template: string, values: Record<string, string>) {
    return Object.entries(values).reduce(
      (out, [key, value]) => out.split(`{${key}}`).join(value),
      template
    );
  }

  function runAdvice(scope: AdviceDataScopeId) {
    scopeOpen = false;
    generateAdvice(scope);
  }

  function confidenceLabel(item: AdviceItemView) {
    return copyTemplate(snapshot.copy.insights.advice_confidence, {
      confidence: `${Math.round(item.confidence * 100)}%`
    });
  }

  function runLabel(run: AdviceRunView) {
    const scope =
      run.data_scope === 'prompt_snippets'
        ? snapshot.copy.insights.advice_scope_snippets
        : snapshot.copy.insights.advice_scope_redacted;
    const template = run.id === latestRun?.id
      ? snapshot.copy.insights.advice_latest
      : snapshot.copy.insights.advice_run;
    return copyTemplate(template, {
      tool: run.tool_label,
      scope,
      status: run.status
    });
  }

  function latestRunLabel() {
    return latestRun ? runLabel(latestRun) : '';
  }

  function nextStepLabel(step: string) {
    return copyTemplate(snapshot.copy.insights.advice_next_step, { step });
  }

  function evidenceLabel(evidence: string[]) {
    return copyTemplate(snapshot.copy.insights.advice_evidence, {
      evidence: evidence.join(' · ')
    });
  }
</script>

<section class="page insights-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <header class="insights-header">
    <div class="title">
      <h2>{snapshot.copy.insights.title}</h2>
      <p>{snapshot.copy.insights.subtitle}</p>
    </div>
    <button
      class="generate-button"
      type="button"
      disabled={snapshot.advice_running}
      aria-busy={snapshot.advice_running}
      onclick={() => (scopeOpen = !scopeOpen)}
    >
      <Sparkles size={16} /> {snapshot.copy.actions.generate_advice}
    </button>
    <dl class="kpis">
      <div>
        <dt>{snapshot.copy.insights.kpi_savings}</dt>
        <dd class="amount">{view.summary.total_est_savings}</dd>
      </div>
      <div>
        <dt>{snapshot.copy.insights.kpi_risks}</dt>
        <dd class="risk">{severityCount('risk')}</dd>
      </div>
      <div>
        <dt>{snapshot.copy.insights.kpi_warns}</dt>
        <dd class="warn">{severityCount('warn')}</dd>
      </div>
      <div>
        <dt>{snapshot.copy.insights.kpi_infos}</dt>
        <dd class="info">{severityCount('info')}</dd>
      </div>
    </dl>
  </header>

  {#if scopeOpen}
    <section class="scope-panel" aria-label={snapshot.copy.insights.advice_scope_title}>
      <button type="button" disabled={snapshot.advice_running} onclick={() => runAdvice('redacted')}>
        <strong>{snapshot.copy.insights.advice_scope_redacted}</strong>
        <span>{snapshot.copy.insights.advice_scope_redacted_detail}</span>
      </button>
      <button type="button" disabled={snapshot.advice_running} onclick={() => runAdvice('prompt_snippets')}>
        <strong>{snapshot.copy.insights.advice_scope_snippets}</strong>
        <span>{snapshot.copy.insights.advice_scope_snippets_detail}</span>
      </button>
    </section>
  {/if}

  <section class="advice-section">
    <div class="section-head">
      <h3><Lightbulb size={15} /> {snapshot.copy.insights.advice_title}</h3>
      {#if latestRun}<span>{latestRunLabel()}</span>{/if}
    </div>

    {#if !latestRun}
      <p class="empty">{snapshot.copy.insights.advice_empty}</p>
    {:else}
      <ul class="run-list">
        {#each snapshot.advice.runs as run (run.id)}
          <li class="run-card">
            <div class="run-head">
              <span>{runLabel(run)}</span>
            </div>
            {#if run.summary}
              <p class="run-summary">{run.summary}</p>
            {/if}
            {#if run.status === 'failed'}
              <p class="failed">
                {copyTemplate(snapshot.copy.insights.advice_failed, { error: run.error ?? run.status })}
              </p>
            {/if}
            {#if run.items.length}
              <ul class="advice-list">
                {#each run.items as item (item.id)}
                  <li>
                    <article class="advice-item" class:is-done={item.status === 'done'} class:is-dismissed={item.status === 'dismissed'}>
                      <div class="advice-main">
                        <div class="advice-meta">
                          <span class={`severity ${item.severity}`}>{item.severity}</span>
                          <span>{item.category}</span>
                          <span>{confidenceLabel(item)}</span>
                        </div>
                        <h4>{item.title}</h4>
                        <p>{item.body}</p>
                        <p class="impact">{item.impact}</p>
                        {#if item.evidence.length}
                          <p class="evidence">{evidenceLabel(item.evidence)}</p>
                        {/if}
                        <p class="next-step">{nextStepLabel(item.next_step)}</p>
                      </div>
                      <div class="item-actions">
                        <span class={`status-chip ${item.status}`}>{item.status}</span>
                        {#if item.status === 'done'}
                          <button class="reopen-command" type="button" title={snapshot.copy.actions.mark_open} onclick={() => updateAdviceItemStatus(item.id, 'open')}>
                            <RotateCcw size={14} /> {snapshot.copy.actions.mark_open}
                          </button>
                        {:else}
                          <button class="done-command" type="button" title={snapshot.copy.actions.mark_done} onclick={() => updateAdviceItemStatus(item.id, 'done')}>
                            <Check size={14} /> {snapshot.copy.actions.mark_done}
                          </button>
                        {/if}
                        {#if item.status !== 'dismissed'}
                          <button class="dismiss-command" type="button" title={snapshot.copy.actions.dismiss} onclick={() => updateAdviceItemStatus(item.id, 'dismissed')}>
                            <X size={14} /> {snapshot.copy.actions.dismiss}
                          </button>
                        {/if}
                      </div>
                    </article>
                  </li>
                {/each}
              </ul>
            {:else if run.status !== 'failed'}
              <p class="empty">{snapshot.copy.insights.advice_empty}</p>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="signals-section">
    <div class="section-head">
      <h3>{snapshot.copy.insights.signals_title}</h3>
    </div>
    {#if view.recommendations.length === 0}
      <p class="empty">{snapshot.copy.insights.empty}</p>
    {:else}
      <ul class="cards">
        {#each view.recommendations as rec (rec.id)}
          <li>
            <RecommendationCard recommendation={rec} />
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</section>

<style>
  .insights-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 0;
    min-height: 0;
    overflow: auto;
    scrollbar-gutter: stable;
    overscroll-behavior: contain;
  }
  .insights-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 14px;
    background: #25293d;
    border: 1px solid #414866;
  }
  .generate-button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 32px;
    padding: 0 11px 0 12px;
    border: 1px solid #f5a45b;
    background: #382b25;
    color: #ffd0a0;
    font-size: 12px;
    font-weight: 700;
    white-space: nowrap;
  }
  .generate-button::after {
    content: '';
    width: 5px;
    height: 5px;
    margin-left: 2px;
    background: #4cf2a0;
  }
  .generate-button:hover {
    background: #463124;
    border-color: #ffc06a;
  }
  .generate-button:disabled,
  .scope-panel button:disabled {
    cursor: wait;
    opacity: 0.62;
  }
  .generate-button:active,
  .scope-panel button:active,
  .item-actions button:active {
    transform: translateY(1px);
  }
  .title h2 {
    margin: 0 0 4px;
    font-size: 16px;
    font-weight: 700;
    color: #cbd4f2;
  }
  .title p {
    margin: 0;
    color: #a1a7c3;
    font-size: 12px;
  }
  .kpis {
    display: flex;
    gap: 18px;
    margin: 0;
  }
  .scope-panel {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    padding: 8px;
    background: #1e2233;
    border: 1px solid #414866;
  }
  .scope-panel button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    min-height: 54px;
    padding: 9px 10px 9px 12px;
    border: 1px solid #414866;
    border-left: 3px solid #f5a45b;
    background: #25293d;
    color: #cbd4f2;
    text-align: left;
  }
  .scope-panel button:hover {
    border-color: #f5a45b;
    background: #2a2f46;
  }
  .scope-panel strong {
    font-size: 13px;
  }
  .scope-panel span {
    font-size: 11px;
    color: #a1a7c3;
  }
  .advice-section,
  .signals-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 28px;
  }
  .section-head h3 {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: #cbd4f2;
    font-size: 13px;
  }
  .section-head span {
    color: #6e7492;
    font-size: 11px;
  }
  .run-summary,
  .failed {
    margin: 0;
    padding: 10px 12px;
    background: #25293d;
    border: 1px solid #414866;
    border-left: 3px solid #4cf2a0;
    color: #cbd4f2;
    font-size: 12px;
    line-height: 1.5;
  }
  .failed {
    border-color: #7a3341;
    color: #ff9aa4;
  }
  .run-list,
  .advice-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .run-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .run-head {
    display: flex;
    justify-content: flex-start;
    color: #6e7492;
    font-size: 11px;
  }
  .advice-item {
    position: relative;
    display: block;
    padding: 12px 12px 12px 14px;
    background: #25293d;
    border: 1px solid #414866;
    border-left: 3px solid #f5a45b;
  }
  .advice-item.is-done {
    opacity: 0.72;
    border-left-color: #4cf2a0;
  }
  .advice-item.is-dismissed {
    opacity: 0.5;
    border-left-color: #6e7492;
  }
  .advice-main {
    min-width: 0;
    padding-right: 218px;
  }
  .advice-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 6px;
    color: #a1a7c3;
    font-size: 11px;
  }
  .severity {
    color: #cbd4f2;
    text-transform: uppercase;
    font-weight: 700;
  }
  .severity.risk {
    color: #ff5f6d;
  }
  .severity.warn {
    color: #ff8f40;
  }
  .severity.info {
    color: #78c6ff;
  }
  .advice-item h4 {
    margin: 0 0 5px;
    color: #eef3ff;
    font-size: 14px;
  }
  .advice-item p {
    margin: 0;
    color: #cbd4f2;
    font-size: 12px;
    line-height: 1.45;
  }
  .advice-item .impact,
  .advice-item .evidence,
  .advice-item .next-step {
    margin-top: 6px;
    color: #a1a7c3;
  }
  .advice-item .evidence {
    overflow-wrap: anywhere;
  }
  .item-actions {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 3px;
    max-width: calc(100% - 20px);
    padding: 3px;
    background: #1e2233;
    border: 1px solid #30354c;
  }
  .item-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 24px;
    padding: 0 7px;
    border: 0;
    background: transparent;
    color: #cbd4f2;
    font-size: 11px;
    white-space: nowrap;
  }
  .item-actions button:hover {
    background: #25293d;
    color: #eef3ff;
  }
  .done-command {
    color: #4cf2a0 !important;
  }
  .reopen-command {
    color: #78c6ff !important;
  }
  .dismiss-command {
    color: #a1a7c3 !important;
  }
  .status-chip {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 0 7px;
    background: #25293d;
    color: #a1a7c3;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
  }
  .status-chip.done {
    color: #4cf2a0;
  }
  .status-chip.dismissed {
    color: #6e7492;
  }
  .kpis div {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    min-width: 90px;
  }
  .kpis dt {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #6e7492;
    margin: 0;
  }
  .kpis dd {
    margin: 0;
    font-weight: 700;
    font-size: 16px;
    color: #cbd4f2;
  }
  .kpis dd.amount {
    color: #ffd60a;
  }
  .kpis dd.risk {
    color: #ff5f6d;
  }
  .kpis dd.warn {
    color: #ff8f40;
  }
  .kpis dd.info {
    color: #a1a7c3;
  }
  .cards {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .empty {
    margin: 0;
    padding: 24px;
    background: #25293d;
    border: 1px solid #414866;
    color: #a1a7c3;
    text-align: center;
  }
  @media (max-width: 760px) {
    .insights-header,
    .advice-item {
      grid-template-columns: 1fr;
    }
    .insights-header {
      flex-direction: column;
    }
    .kpis,
    .scope-panel {
      width: 100%;
    }
    .scope-panel {
      grid-template-columns: 1fr;
    }
    .advice-main {
      padding-right: 0;
    }
    .item-actions {
      position: static;
      margin-top: 10px;
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
</style>
