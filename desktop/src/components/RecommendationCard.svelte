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
    gap: 6px;
    padding: 10px 12px;
    background: #25293d;
    border: 1px solid #414866;
    border-left-width: 3px;
  }
  .recommendation.severity-risk {
    border-left-color: #ff5f6d;
  }
  .recommendation.severity-warn {
    border-left-color: #ff8f40;
  }
  .recommendation.severity-info {
    border-left-color: #6e7492;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #a1a7c3;
  }
  .severity-badge {
    color: #cbd4f2;
    background: #292d44;
    border: 1px solid #414866;
    padding: 1px 6px;
  }
  .severity-risk .severity-badge {
    color: #ff5f6d;
    border-color: #ff5f6d;
  }
  .severity-warn .severity-badge {
    color: #ff8f40;
    border-color: #ff8f40;
  }
  .scope {
    color: #6e7492;
  }
  .savings {
    margin-left: auto;
    color: #ffd60a;
    font-weight: 700;
    text-transform: none;
    letter-spacing: 0;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 700;
    color: #cbd4f2;
  }
  .body {
    margin: 0;
    color: #cbd4f2;
    font-size: 13px;
    line-height: 1.5;
  }
  .silenced {
    margin: 0;
    color: #a1a7c3;
    font-size: 12px;
    line-height: 1.4;
  }
  .assumption {
    margin: 0;
    color: #6e7492;
    font-size: 12px;
    line-height: 1.4;
  }
</style>
