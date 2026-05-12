<script lang="ts">
  import { ArrowLeft, Check, CircleDot, RotateCcw, X } from 'lucide-svelte';
  import { fadeIn } from '../../motion';
  import type { AdviceItemStatusId, DesktopSnapshot } from '../../types';
  import type { InsightRow, InsightsCopy } from './model';

  export let snapshot: DesktopSnapshot;
  export let row: InsightRow | null;
  export let copy: InsightsCopy;
  export let closeDetail: () => void;
  export let markAdvice: (row: InsightRow, status: AdviceItemStatusId) => void;
</script>

<section class="screen detail-screen" use:fadeIn>
  {#if row}
    <article class={`detail-page severity-${row.severity}`}>
      <header class="detail-hero">
        <button type="button" class="back-button" onclick={closeDetail}>
          <ArrowLeft size={15} /> {copy.detail_back}
        </button>
        <div>
          <span class="eyebrow">{row.sourceLabel}</span>
          <h3>{row.title}</h3>
          <p>{row.scopeLabel}</p>
        </div>
        <div class="detail-badges">
          <span class={`severity-label ${row.severity}`}>{row.severityLabel}</span>
          <span class={`status-chip status-${row.status}`}>{row.statusLabel}</span>
        </div>
      </header>

      <div class="detail-layout">
        <section class="detail-main">
          <section class="detail-block">
            <span>{copy.detail_observation}</span>
            <p>{row.body}</p>
          </section>

          {#if row.impact}
            <section class="detail-block impact">
              <span>{copy.detail_impact}</span>
              <p>{row.impact}</p>
            </section>
          {/if}

          {#if row.nextStep}
            <section class="detail-block next-step">
              <span>{copy.detail_next_step}</span>
              <p>{row.nextStep}</p>
            </section>
          {/if}

          {#if row.assumption || row.silencedReason}
            <section class="detail-block">
              <span>{copy.detail_guardrails}</span>
              {#if row.assumption}<p>{row.assumption}</p>{/if}
              {#if row.silencedReason}<p>{row.silencedReason}</p>{/if}
            </section>
          {/if}
        </section>

        <aside class="detail-side">
          <section class="detail-block">
            <span>{copy.detail_scope}</span>
            <dl class="detail-facts">
              <div>
                <dt>{copy.detail_status}</dt>
                <dd>{row.statusLabel}</dd>
              </div>
              <div>
                <dt>{copy.detail_scope}</dt>
                <dd>{row.scopeLabel}</dd>
              </div>
              {#if row.savings}
                <div>
                  <dt>{copy.detail_savings}</dt>
                  <dd>{row.savings}</dd>
                </div>
              {/if}
              {#if row.ruleId}
                <div>
                  <dt>{copy.detail_rule}</dt>
                  <dd>{row.ruleId}</dd>
                </div>
              {/if}
            </dl>
          </section>

          {#if row.evidence.length}
            <section class="detail-block">
              <span>{copy.detail_evidence}</span>
              <div class="evidence-list">
                {#each row.evidence as evidence}
                  <code>{evidence}</code>
                {/each}
              </div>
            </section>
          {/if}

          {#if row.advice}
            <section class="detail-actions">
              {#if row.advice.status === 'open'}
                <button class="done-command" type="button" title={snapshot.copy.actions.mark_done} onclick={() => markAdvice(row, 'done')}>
                  <Check size={14} /> {snapshot.copy.actions.mark_done}
                </button>
                <button class="dismiss-command" type="button" title={snapshot.copy.actions.dismiss} onclick={() => markAdvice(row, 'dismissed')}>
                  <X size={14} /> {snapshot.copy.actions.dismiss}
                </button>
              {:else}
                <button class="reopen-command" type="button" title={snapshot.copy.actions.mark_open} onclick={() => markAdvice(row, 'open')}>
                  <RotateCcw size={14} /> {snapshot.copy.actions.mark_open}
                </button>
              {/if}
            </section>
          {/if}
        </aside>
      </div>
    </article>
  {:else}
    <div class="empty-state screen-empty">
      <CircleDot size={18} />
      <strong>{copy.detail_empty_title}</strong>
      <p>{copy.detail_empty_detail}</p>
    </div>
  {/if}
</section>
