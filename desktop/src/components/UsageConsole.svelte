<script lang="ts">
  import Panel from '../Panel.svelte';
  import { staggeredReveal } from '../motion';
  import type { CopyDeck, RecentModelMetric, ToolLimitSection } from '../types';
  import GaugeBar from './GaugeBar.svelte';
  import RankBar from './RankBar.svelte';
  import UsageActivityChart from './UsageActivityChart.svelte';

  export let section: ToolLimitSection;
  export let tone = 'cyan';
  export let copy: CopyDeck;

  $: buckets = section.usage.buckets;
  $: hasUsage = section.usage.calls > 0 || buckets.some((bucket) => bucket > 0);
  $: modelRows = section.models.slice(0, 3);

  function count(value: number) {
    return value.toLocaleString();
  }

  function modelLabel(model: RecentModelMetric) {
    return `${model.name}: ${count(model.calls)} ${copy.metrics.calls}`;
  }

  function credit(value: number) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 2 });
  }
</script>

<Panel title={copy.usage.console_title.replace('{tool}', section.tool)} {tone}>
  <div class="usage-console" use:staggeredReveal={{ selector: '.console-pulse, .console-stats div, .console-row', y: 3, stagger: 0.012 }}>
    <div class="console-head">
      <div class="console-pulse">
        <UsageActivityChart {buckets} active={hasUsage} {tone} ariaLabel={`${section.tool} ${copy.tray.hours_24} ${copy.timeline.pulse}`} />
      </div>

      <div class="console-stats">
        <div><span>{copy.metrics.cost}</span><strong class="money">{section.usage.cost}</strong></div>
        <div><span>{copy.metrics.calls}</span><strong>{count(section.usage.calls)}</strong></div>
        <div><span>{copy.metrics.tokens}</span><strong>{section.usage.tokens}</strong></div>
        <div><span>{copy.usage.seen}</span><strong>{section.usage.last_seen}</strong></div>
      </div>
    </div>

    <div class="console-table">
      <div class="console-row console-labels">
        <span>{copy.tables.kind}</span>
        <span>{copy.tables.scope_model_spaced}</span>
        <span>{copy.tables.used}</span>
        <span>{copy.tables.left_calls_spaced}</span>
        <span>{copy.tables.reset_tokens_spaced}</span>
        <span>{copy.tables.cost_plan_spaced}</span>
      </div>

      {#each section.limits as limit}
        <div class="console-row limit-row">
          <strong>{copy.usage.limit}</strong>
          <span>{limit.scope} {limit.window}</span>
          <div class="limit-used">
            <GaugeBar used={limit.used} ariaLabel={`${limit.scope} ${limit.window}`} usedSuffix={copy.usage.used_suffix} />
            {#if limit.used_credits !== null}
              <span class="credit-number">{credit(limit.used_credits)} {copy.usage.credits_short}</span>
            {/if}
          </div>
          {#if limit.remaining_credits !== null && limit.total_credits !== null}
            <span class="credit-balance">
              <strong>{credit(limit.remaining_credits)}</strong>
              <small>/ {credit(limit.total_credits)} {copy.usage.credits_short}</small>
            </span>
          {:else}
            <span>{limit.left}</span>
          {/if}
          <span>{limit.reset}</span>
          <span class="plan-state">
            <span>{limit.plan}</span>
            {#if limit.additional_usage !== null}
              <small class:enabled={limit.additional_usage}>
                {limit.additional_usage ? copy.usage.additional_enabled : copy.usage.additional_disabled}
              </small>
            {/if}
          </span>
        </div>
      {/each}

      {#each modelRows as model}
        <div class="console-row">
          <strong>{copy.usage.model}</strong>
          <span>{model.name}</span>
          <RankBar value={model.value} ariaLabel={modelLabel(model)} compact />
          <span>{count(model.calls)}</span>
          <span>{model.tokens}</span>
          <span class="money">{model.cost}</span>
        </div>
      {/each}

      {#if !section.limits.length && !modelRows.length}
        <div class="console-empty">{copy.usage.idle}</div>
      {/if}
    </div>
  </div>
</Panel>

<style>
  .usage-console {
    min-height: 0;
    display: grid;
    gap: 12px;
  }

  .console-head {
    display: grid;
    grid-template-columns: minmax(200px, 1fr) minmax(260px, 0.88fr);
    gap: 10px;
    align-items: stretch;
  }

  .console-pulse {
    min-height: 64px;
    min-width: 0;
  }

  .console-stats {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .console-stats div {
    min-width: 0;
    border: 1px solid var(--color-border-row);
    padding: 6px 7px;
    display: grid;
    align-content: center;
    gap: 1px;
  }

  .console-stats span,
  .console-labels {
    color: var(--color-muted);
    text-transform: uppercase;
    font-size: 12px;
  }

  .console-stats strong,
  .console-row span,
  .console-row strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .console-table {
    min-width: 0;
    display: grid;
  }

  .console-row {
    min-width: 0;
    min-height: 30px;
    display: grid;
    grid-template-columns: 70px minmax(140px, 1fr) minmax(126px, 0.95fr) minmax(126px, 0.9fr) minmax(110px, 0.8fr) minmax(128px, 0.95fr);
    gap: 8px;
    align-items: center;
    border-bottom: 1px solid var(--color-border-row);
  }

  .console-row strong:first-child,
  .limit-row strong:first-child {
    color: var(--color-cyan);
  }

  .limit-row {
    color: var(--color-muted);
  }

  .limit-used {
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(74px, 1fr) auto;
    gap: 7px;
    align-items: center;
  }

  .credit-number,
  .credit-balance,
  .plan-state {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }

  .credit-number {
    color: var(--color-on-surface);
    font-size: 12px;
  }

  .credit-balance {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
  }

  .credit-balance strong {
    color: var(--color-on-surface);
    font-weight: 650;
  }

  .credit-balance small,
  .plan-state small {
    color: var(--color-muted);
    font-size: 11px;
  }

  .plan-state {
    display: grid;
    gap: 1px;
  }

  .plan-state > span {
    color: var(--color-warning);
  }

  .plan-state small.enabled {
    color: var(--color-tertiary);
  }

  .money {
    color: var(--color-warning);
  }

  .console-empty {
    min-height: 42px;
    display: grid;
    align-items: center;
    color: var(--color-muted);
    border-bottom: 1px solid var(--color-border-row);
  }

  @media (max-width: 980px) {
    .console-head {
      grid-template-columns: minmax(0, 1fr);
    }

    .console-stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .console-row {
      grid-template-columns: 58px minmax(120px, 1fr) 78px 82px;
    }

    .console-row > :nth-child(5),
    .console-row > :nth-child(6) {
      display: none;
    }
  }
</style>
