<script lang="ts">
  import { Download, RefreshCw } from 'lucide-svelte';
  import type { CopyDeck, DesktopSnapshot, PeriodId, SortId, ToolId } from '../types';

  export let copy: CopyDeck;
  export let title: string;
  export let snapshot: DesktopSnapshot;
  export let showPeriod = true;
  export let showTool = true;
  export let showSort = true;
  export let showProject = true;
  export let periodLocked: PeriodId | null = null;
  export let statusMessage: string | null = null;
  export let statusTone = 'info';
  export let setPeriod: (period: PeriodId) => void;
  export let setTool: (event: Event) => void;
  export let setSort: (event: Event) => void;
  export let openProjectPicker: () => void;
  export let refresh: () => void;
  export let openReport: () => void;

  let statusExpanded = false;
</script>

<header class="page-header">
  <div class="page-header-row">
    <h1 class="page-title">{title}</h1>
    <div class="page-actions">
      {#if statusMessage}
        <button
          class="status-pill"
          class:error={statusTone === 'error'}
          class:success={statusTone === 'success'}
          class:warning={statusTone === 'warning'}
          class:busy={statusTone === 'busy'}
          class:is-expanded={statusExpanded}
          type="button"
          title={statusExpanded ? '' : statusMessage}
          onclick={() => (statusExpanded = !statusExpanded)}
        >
          <i class="status-dot" aria-hidden="true"></i>
          <span>{statusMessage}</span>
        </button>
      {/if}
      <button class="icon-button" type="button" title={copy.actions.refresh_archive} onclick={refresh}>
        <RefreshCw size={16} />
      </button>
      <button class="icon-button" type="button" title={copy.actions.export_current_view} onclick={openReport}>
        <Download size={16} />
      </button>
    </div>
  </div>

  {#if showPeriod || showTool || showSort || showProject}
    <div class="page-filters">
      {#if showPeriod}
        <div class="segmented" aria-label={copy.desktop.period_aria}>
          {#each snapshot.periods as period}
            <button
              type="button"
              class:active={(periodLocked ?? snapshot.period) === period.value}
              disabled={periodLocked !== null && period.value !== periodLocked}
              onclick={() => setPeriod(period.value)}
            >
              {period.label}
            </button>
          {/each}
        </div>
      {/if}

      <div class="filter-controls">
        {#if showTool}
          <div class="segmented tool-strip" aria-label={copy.desktop.tool_aria}>
            <span>{copy.filters.tool}</span>
            <select aria-label={copy.desktop.tool_aria} onchange={setTool}>
              {#each snapshot.tools as tool}
                <option value={tool.value} selected={snapshot.tool === tool.value}>{tool.label}</option>
              {/each}
            </select>
          </div>
        {/if}

        {#if showSort}
          <div class="segmented tool-strip sort-strip" aria-label={copy.desktop.sort_aria}>
            <span>{copy.filters.sort}</span>
            <select aria-label={copy.desktop.sort_aria} onchange={setSort}>
              {#each snapshot.sorts as sort}
                <option value={sort.value} selected={snapshot.sort === sort.value}>{sort.label}</option>
              {/each}
            </select>
          </div>
        {/if}

        {#if showProject}
          <button
            class="segmented tool-strip project-strip"
            type="button"
            aria-label={copy.desktop.project_aria}
            onclick={openProjectPicker}
          >
            <span>{copy.filters.project}</span>
            <strong>{snapshot.project.label}</strong>
          </button>
        {/if}
      </div>
    </div>
  {/if}
</header>
