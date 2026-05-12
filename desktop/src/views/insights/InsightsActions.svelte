<script lang="ts">
  import { Check, ChevronRight, CircleDot, RotateCcw, X } from 'lucide-svelte';
  import { fadeIn, staggeredReveal } from '../../motion';
  import type { AdviceItemStatusId, DesktopSnapshot } from '../../types';
  import {
    actionFilterIds,
    actionFilterLabel,
    rowMatchesActionFilter,
    type ActionFilter,
    type InsightModel,
    type InsightRow
  } from './model';

  export let snapshot: DesktopSnapshot;
  export let model: InsightModel;
  export let rows: InsightRow[];
  export let actionFilter: ActionFilter;
  export let setActionFilter: (filter: ActionFilter) => void;
  export let openDetail: (row: InsightRow, from: 'actions') => void;
  export let markAdvice: (row: InsightRow, status: AdviceItemStatusId) => void;

  function actionFilterCount(filter: ActionFilter) {
    return model.actionRows.filter((row) => rowMatchesActionFilter(row, filter)).length;
  }
</script>

<section class="screen action-screen" use:fadeIn>
  <div class="screen-head">
    <div>
      <span class="eyebrow">{model.copy.screen_actions}</span>
      <h3>{model.copy.actions_title}</h3>
      <p>{model.copy.actions_subtitle}</p>
    </div>
    <div class="filter-tabs" aria-label={model.copy.filter_title}>
      {#each actionFilterIds as filter}
        <button
          type="button"
          class:active={actionFilter === filter}
          aria-pressed={actionFilter === filter}
          onclick={() => setActionFilter(filter)}
        >
          {actionFilterLabel(filter, model.copy)}
          <span>{actionFilterCount(filter)}</span>
        </button>
      {/each}
    </div>
  </div>

  {#if rows.length}
    <div class="action-list" use:staggeredReveal={{ selector: ':scope > *', y: 3, stagger: 0.018 }}>
      {#each rows as row (row.id)}
        <article class={`action-row severity-${row.severity}`} class:is-complete={row.status !== 'open'}>
          <span class="row-rail"></span>
          <div class="row-body">
            <span class="row-meta">
              <strong class={`severity-label ${row.severity}`}>{row.severityLabel}</strong>
              <span>{row.category}</span>
              {#if row.confidence}<span>{row.confidence}</span>{/if}
              <span class={`status-chip status-${row.status}`}>{row.statusLabel}</span>
            </span>
            <h4>{row.title}</h4>
            <p>{row.impact ?? row.body}</p>
            <span class="scope-line">{row.scopeLabel}</span>
          </div>
          <div class="row-actions">
            {#if row.savings}<span class="money-chip">{row.savings}</span>{/if}
            <button type="button" onclick={() => openDetail(row, 'actions')}>
              {model.copy.detail_open}
              <ChevronRight size={14} />
            </button>
            {#if row.advice?.status === 'open'}
              <button class="done-command" type="button" title={snapshot.copy.actions.mark_done} onclick={() => markAdvice(row, 'done')}>
                <Check size={14} /> {snapshot.copy.actions.mark_done}
              </button>
              <button class="dismiss-command" type="button" title={snapshot.copy.actions.dismiss} onclick={() => markAdvice(row, 'dismissed')}>
                <X size={14} /> {snapshot.copy.actions.dismiss}
              </button>
            {:else if row.advice}
              <button class="reopen-command" type="button" title={snapshot.copy.actions.mark_open} onclick={() => markAdvice(row, 'open')}>
                <RotateCcw size={14} /> {snapshot.copy.actions.mark_open}
              </button>
            {/if}
          </div>
        </article>
      {/each}
    </div>
  {:else}
    <div class="empty-state screen-empty">
      <CircleDot size={18} />
      <strong>{model.copy.actions_empty_title}</strong>
      <p>{model.copy.actions_empty_detail}</p>
    </div>
  {/if}
</section>
