<script lang="ts">
  import RecommendationCard from '../components/RecommendationCard.svelte';
  import { staggeredReveal } from '../motion';
  import type { DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;

  $: view = snapshot.insights;
  $: severityCount = (id: string) =>
    view.summary.by_severity.find((s) => s.id === id)?.count ?? 0;
</script>

<section class="page insights-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <header class="insights-header">
    <div class="title">
      <h2>{snapshot.copy.insights.title}</h2>
      <p>{snapshot.copy.insights.subtitle}</p>
    </div>
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

<style>
  .insights-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 0;
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
</style>
