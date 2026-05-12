<script lang="ts">
  import { staggeredReveal } from '../motion';
  import type {
    AdviceDataScopeId,
    AdviceItemStatusId,
    DesktopSnapshot
  } from '../types';
  import InsightDetail from './insights/InsightDetail.svelte';
  import InsightsActions from './insights/InsightsActions.svelte';
  import InsightsHeader from './insights/InsightsHeader.svelte';
  import InsightsOverview from './insights/InsightsOverview.svelte';
  import InsightsRuns from './insights/InsightsRuns.svelte';
  import InsightsSignals from './insights/InsightsSignals.svelte';
  import './insights/insights.css';
  import {
    actionFilterIds,
    createInsightModel,
    rowMatchesActionFilter,
    type ActionFilter,
    type InsightRow,
    type InsightsScreen,
    type MainInsightsScreen
  } from './insights/model';

  export let snapshot: DesktopSnapshot;
  export let generateAdvice: (scope: AdviceDataScopeId) => void;
  export let updateAdviceItemStatus: (itemId: number, status: AdviceItemStatusId) => void;
  export let generateAdviceRequest = 0;

  let activeInsightsScreen: InsightsScreen = 'overview';
  let previousInsightsScreen: MainInsightsScreen = 'overview';
  let selectedInsightId = '';
  let actionFilter: ActionFilter = 'open';
  let selectedAdviceScope: AdviceDataScopeId = 'redacted';
  let handledGenerateAdviceRequest = 0;

  $: model = createInsightModel(snapshot);
  $: selectedRow = model.allRows.find((row) => row.id === selectedInsightId) ?? null;
  $: filteredActionRows = model.actionRows.filter((row) => rowMatchesActionFilter(row, actionFilter));
  $: if (!actionFilterIds.includes(actionFilter)) {
    actionFilter = 'open';
  }
  $: if (activeInsightsScreen === 'detail' && selectedInsightId && !selectedRow) {
    activeInsightsScreen = previousInsightsScreen;
  }
  $: if (generateAdviceRequest > 0 && generateAdviceRequest !== handledGenerateAdviceRequest) {
    handledGenerateAdviceRequest = generateAdviceRequest;
    runSelectedAdvice();
  }

  function showScreen(screen: MainInsightsScreen) {
    activeInsightsScreen = screen;
    previousInsightsScreen = screen;
  }

  function openDetail(row: InsightRow, from: MainInsightsScreen = screenForDetailBack()) {
    selectedInsightId = row.id;
    previousInsightsScreen = from;
    activeInsightsScreen = 'detail';
  }

  function screenForDetailBack(): MainInsightsScreen {
    return activeInsightsScreen === 'detail' ? previousInsightsScreen : activeInsightsScreen;
  }

  function closeDetail() {
    activeInsightsScreen = previousInsightsScreen;
  }

  function setAdviceScope(scope: AdviceDataScopeId) {
    selectedAdviceScope = scope;
  }

  function runSelectedAdvice() {
    if (snapshot.advice_running) return;
    generateAdvice(selectedAdviceScope);
  }

  function markAdvice(row: InsightRow, status: AdviceItemStatusId) {
    if (!row.advice) return;
    updateAdviceItemStatus(row.advice.id, status);
  }
</script>

<section class="page insights-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <InsightsHeader
    {snapshot}
    {activeInsightsScreen}
    {previousInsightsScreen}
    {showScreen}
    {selectedAdviceScope}
    {setAdviceScope}
    {runSelectedAdvice}
    copy={model.copy}
  />

  <div class="screen-shell">
    {#if activeInsightsScreen === 'overview'}
      <InsightsOverview {snapshot} {model} {openDetail} {showScreen} />
    {:else if activeInsightsScreen === 'actions'}
      <InsightsActions
        {snapshot}
        {model}
        rows={filteredActionRows}
        {actionFilter}
        setActionFilter={(filter) => (actionFilter = filter)}
        {openDetail}
        {markAdvice}
      />
    {:else if activeInsightsScreen === 'signals'}
      <InsightsSignals {snapshot} {model} {openDetail} />
    {:else if activeInsightsScreen === 'runs'}
      <InsightsRuns
        {snapshot}
        {model}
        {selectedAdviceScope}
        {setAdviceScope}
        {runSelectedAdvice}
      />
    {:else if activeInsightsScreen === 'detail'}
      <InsightDetail {snapshot} row={selectedRow} {closeDetail} {markAdvice} copy={model.copy} />
    {/if}
  </div>
</section>
