<script lang="ts">
  import { reveal } from '../motion';
  import type { RecommendationView } from '../types';

  export let recommendation: RecommendationView;
</script>

<article
  class={`recommendation severity-${recommendation.severity}`}
  data-rule={recommendation.rule_id}
  use:reveal={{ y: 3 }}
>
  <header>
    <span class="severity-badge">{recommendation.severity_label}</span>
    <span class="category">{recommendation.category_label}</span>
    {#if recommendation.scope.label}
      <span class="scope">{recommendation.scope.label}</span>
    {/if}
    {#if recommendation.savings}
      <span class="savings">{recommendation.savings}</span>
    {/if}
  </header>
  <h3>{recommendation.title}</h3>
  {#if recommendation.body}
    <p class="body">{recommendation.body}</p>
  {/if}
  {#if recommendation.silenced_reason}
    <p class="silenced">{recommendation.silenced_reason}</p>
  {/if}
  {#if recommendation.assumption}
    <p class="assumption">{recommendation.assumption}</p>
  {/if}
</article>

<style>
  .recommendation {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px 12px 16px;
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-left-width: 3px;
    border-radius: var(--radius-md);
    transition: border-color var(--motion-fast) var(--ease-standard);
  }
  .recommendation:hover {
    border-color: var(--color-border);
  }
  .recommendation.severity-risk {
    border-left-color: var(--color-error);
  }
  .recommendation.severity-warn {
    border-left-color: var(--color-primary);
  }
  .recommendation.severity-info {
    border-left-color: var(--color-muted-2);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-family: var(--font-ui);
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--color-muted);
  }
  .severity-badge {
    color: var(--color-on-surface);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-pill);
    padding: 1px 8px;
    font-size: 10px;
  }
  .severity-risk .severity-badge {
    color: var(--color-error);
    border-color: rgba(255, 95, 109, 0.45);
  }
  .severity-warn .severity-badge {
    color: var(--color-primary);
    border-color: rgba(255, 143, 64, 0.45);
  }
  .scope {
    color: var(--color-muted-2);
  }
  .savings {
    margin-left: auto;
    color: var(--color-warning);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    text-transform: none;
    letter-spacing: 0;
  }
  h3 {
    margin: 0;
    font-family: var(--font-ui);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.005em;
    color: var(--color-on-surface);
    line-height: 1.3;
  }
  .body {
    margin: 0;
    color: var(--color-on-surface);
    font-family: var(--font-ui);
    font-size: 12.5px;
    line-height: 1.55;
  }
  .silenced {
    margin: 0;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 11.5px;
    line-height: 1.45;
  }
  .assumption {
    margin: 0;
    color: var(--color-muted-2);
    font-family: var(--font-ui);
    font-size: 11.5px;
    line-height: 1.45;
  }
</style>
