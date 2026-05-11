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
                          {#if item.status !== 'open'}
                            <span class={`status-chip ${item.status}`}>{item.status}</span>
                          {/if}
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
                        {#if item.status === 'open'}
                          <button class="done-command" type="button" title={snapshot.copy.actions.mark_done} onclick={() => updateAdviceItemStatus(item.id, 'done')}>
                            <Check size={14} /> {snapshot.copy.actions.mark_done}
                          </button>
                          <button class="dismiss-command" type="button" title={snapshot.copy.actions.dismiss} onclick={() => updateAdviceItemStatus(item.id, 'dismissed')}>
                            <X size={14} /> {snapshot.copy.actions.dismiss}
                          </button>
                        {:else}
                          <button class="reopen-command" type="button" title={snapshot.copy.actions.mark_open} onclick={() => updateAdviceItemStatus(item.id, 'open')}>
                            <RotateCcw size={14} /> {snapshot.copy.actions.mark_open}
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
    gap: var(--space-lg);
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
    gap: var(--space-xl);
    padding: var(--space-lg) var(--space-xl);
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-md);
  }
  .generate-button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 34px;
    padding: 0 14px;
    border: 1px solid var(--color-primary);
    background: var(--color-primary);
    color: var(--color-surface);
    border-radius: var(--radius-sm);
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.01em;
    white-space: nowrap;
  }
  .generate-button::after {
    content: '';
    width: 6px;
    height: 6px;
    margin-left: 2px;
    border-radius: 50%;
    background: var(--color-tertiary);
  }
  .generate-button:hover {
    background: #ffa05c;
    border-color: #ffa05c;
    color: var(--color-surface);
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
    font-family: var(--font-ui);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--color-on-surface);
  }
  .title p {
    margin: 0;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 12px;
    line-height: 1.5;
  }
  .kpis {
    display: flex;
    gap: var(--space-xl);
    margin: 0;
  }
  .scope-panel {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-md);
    padding: var(--space-md);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-md);
  }
  .scope-panel button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    min-height: 56px;
    padding: 10px 12px;
    border: 1px solid var(--color-border-soft);
    border-left: 3px solid var(--color-primary);
    border-radius: var(--radius-sm);
    background: var(--color-neutral);
    color: var(--color-on-surface);
    text-align: left;
    font-family: var(--font-ui);
  }
  .scope-panel button:hover {
    border-color: var(--color-primary);
    background: var(--color-neutral-hover);
  }
  .scope-panel strong {
    font-size: 13px;
    font-weight: 600;
  }
  .scope-panel span {
    font-size: 11px;
    color: var(--color-muted);
  }
  .advice-section,
  .signals-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-lg);
    min-height: 28px;
    padding: 0 var(--space-xs);
  }
  .section-head h3 {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--color-on-surface);
    font-family: var(--font-ui);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .section-head span {
    color: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 11px;
  }
  .run-summary {
    margin: 0;
    padding: 12px 14px;
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-left: 3px solid var(--color-tertiary);
    border-radius: var(--radius-sm);
    color: var(--color-on-surface);
    font-family: var(--font-ui);
    font-size: 12.5px;
    line-height: 1.55;
  }
  /* Inline notice for a failed run — calm, not alarming. */
  .failed {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px 12px;
    background: rgba(255, 95, 109, 0.06);
    border: 1px solid rgba(255, 95, 109, 0.32);
    border-radius: var(--radius-sm);
    color: var(--color-error);
    font-family: var(--font-ui);
    font-size: 12px;
    line-height: 1.4;
  }
  .failed::before {
    content: '';
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-error);
  }
  .run-list,
  .advice-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }
  .run-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }
  .run-head {
    display: flex;
    justify-content: flex-start;
    color: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 11px;
  }
  .advice-item {
    position: relative;
    display: block;
    padding: 14px 14px 14px 16px;
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-left: 3px solid var(--color-primary);
    border-radius: var(--radius-md);
    transition: border-color var(--motion-fast) var(--ease-standard);
  }
  .advice-item:hover {
    border-color: var(--color-border);
  }
  .advice-item.is-done {
    opacity: 0.7;
    border-left-color: var(--color-tertiary);
  }
  .advice-item.is-dismissed {
    opacity: 0.5;
    border-left-color: var(--color-muted-2);
  }
  .advice-main {
    min-width: 0;
    padding-right: 156px;
  }
  .advice-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 11px;
  }
  .severity {
    font-family: var(--font-ui);
    font-weight: 700;
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-on-surface);
  }
  .severity.risk {
    color: var(--color-error);
  }
  .severity.warn {
    color: var(--color-primary);
  }
  .severity.info {
    color: var(--color-secondary);
  }
  .advice-item h4 {
    margin: 0 0 6px;
    color: #eef3ff;
    font-family: var(--font-ui);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.005em;
    line-height: 1.3;
  }
  .advice-item p {
    margin: 0;
    color: var(--color-on-surface);
    font-family: var(--font-ui);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .advice-item .impact,
  .advice-item .next-step {
    margin-top: 8px;
    color: var(--color-muted);
  }
  .advice-item .evidence {
    margin-top: 8px;
    color: var(--color-muted);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-size: 11px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }
  .item-actions {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    align-items: center;
    gap: 3px;
    max-width: calc(100% - 24px);
    padding: 3px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-sm);
  }
  .item-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-height: 24px;
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: var(--color-on-surface);
    font-family: var(--font-ui);
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
  }
  .item-actions button:hover {
    background: var(--color-neutral);
    color: #eef3ff;
  }
  .done-command {
    color: var(--color-tertiary) !important;
  }
  .reopen-command {
    color: var(--color-secondary) !important;
  }
  .dismiss-command {
    color: var(--color-muted) !important;
  }
  .status-chip {
    display: inline-flex;
    align-items: center;
    height: 16px;
    padding: 0 6px;
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    line-height: 1;
    text-transform: uppercase;
  }
  .status-chip.done {
    color: var(--color-tertiary);
  }
  .status-chip.dismissed {
    color: var(--color-muted-2);
  }
  .kpis div {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    min-width: 92px;
  }
  .kpis dt {
    font-family: var(--font-ui);
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--color-muted);
    margin: 0;
  }
  .kpis dd {
    margin: 0;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    font-size: 17px;
    color: var(--color-on-surface);
    letter-spacing: -0.01em;
  }
  .kpis dd.amount {
    color: var(--color-warning);
  }
  .kpis dd.risk {
    color: var(--color-error);
  }
  .kpis dd.warn {
    color: var(--color-primary);
  }
  .kpis dd.info {
    color: var(--color-muted);
  }
  .cards {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }
  .empty {
    margin: 0;
    padding: 28px;
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-md);
    color: var(--color-muted);
    font-family: var(--font-ui);
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
