<script lang="ts">
  import { FileText, LayoutDashboard, ListChecks, BarChart3 } from 'lucide-svelte';
  import type { AdviceDataScopeId, DesktopSnapshot } from '../../types';
  import AdviceGenerateControl from './AdviceGenerateControl.svelte';
  import {
    screenIds,
    screenLabel,
    type InsightsCopy,
    type InsightsScreen,
    type MainInsightsScreen
  } from './model';

  export let snapshot: DesktopSnapshot;
  export let copy: InsightsCopy;
  export let activeInsightsScreen: InsightsScreen;
  export let previousInsightsScreen: MainInsightsScreen;
  export let showScreen: (screen: MainInsightsScreen) => void;
  export let selectedAdviceScope: AdviceDataScopeId;
  export let setAdviceScope: (scope: AdviceDataScopeId) => void;
  export let runSelectedAdvice: () => void;

  function isActive(screen: MainInsightsScreen) {
    return activeInsightsScreen === screen || (activeInsightsScreen === 'detail' && previousInsightsScreen === screen);
  }
</script>

<header class="insights-header">
  <div class="insights-title">
    <span class="eyebrow">{copy.dashboard_label}</span>
    <h2>{copy.title}</h2>
  </div>

  <nav class="screen-tabs" aria-label={copy.screen_nav_label}>
    {#each screenIds as screen}
      <button
        type="button"
        class:active={isActive(screen)}
        aria-current={activeInsightsScreen === screen ? 'page' : undefined}
        onclick={() => showScreen(screen)}
      >
        {#if screen === 'overview'}<LayoutDashboard size={14} />{/if}
        {#if screen === 'actions'}<ListChecks size={14} />{/if}
        {#if screen === 'signals'}<BarChart3 size={14} />{/if}
        {#if screen === 'runs'}<FileText size={14} />{/if}
        {screenLabel(screen, copy)}
      </button>
    {/each}
  </nav>

  <AdviceGenerateControl
    {copy}
    adviceRunning={snapshot.advice_running}
    {selectedAdviceScope}
    {setAdviceScope}
    {runSelectedAdvice}
    controlId="header"
  />
</header>
