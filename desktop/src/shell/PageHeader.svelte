<script lang="ts">
  import { ArrowUpDown, Download, FolderGit2, RefreshCw, Wrench } from 'lucide-svelte';
  import type { CopyDeck, DesktopSnapshot, PeriodId, SortId, ToolId } from '../types';

  export let copy: CopyDeck;
  export let title: string;
  export let snapshot: DesktopSnapshot;
  export let showPeriod = true;
  export let showTool = true;
  export let showSort = true;
  export let showProject = true;
  export let setPeriod: (period: PeriodId) => void;
  export let setTool: (event: Event) => void;
  export let setSort: (event: Event) => void;
  export let openProjectPicker: () => void;
  export let refresh: () => void;
  export let openReport: () => void;

</script>

<header class="page-header">
  <div class="page-header-row">
    <div class="page-heading-cluster">
      <h1 class="page-title">{title}</h1>
      {#if showPeriod}
        <div class="segmented period-segmented" aria-label={copy.desktop.period_aria}>
          {#each snapshot.periods as period}
            <button
              type="button"
              class:active={snapshot.period === period.value}
              onclick={() => setPeriod(period.value)}
            >
              {period.label}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="page-actions">
      {#if showTool || showProject || showSort}
        <div class="filter-controls">
          {#if showTool}
            <label class="filter-field" aria-label={copy.desktop.tool_aria}>
              <span class="filter-label" aria-hidden="true">
                <Wrench size={14} />
                <span class="filter-label-text">{copy.filters.tool}</span>
              </span>
              <select aria-label={copy.desktop.tool_aria} onchange={setTool}>
                {#each snapshot.tools as tool}
                  <option value={tool.value} selected={snapshot.tool === tool.value}>{tool.label}</option>
                {/each}
              </select>
            </label>
          {/if}

          {#if showProject}
            <button
              class="filter-field project-filter"
              type="button"
              aria-label={copy.desktop.project_aria}
              onclick={openProjectPicker}
            >
              <span class="filter-label" aria-hidden="true">
                <FolderGit2 size={14} />
                <span class="filter-label-text">{copy.filters.project}</span>
              </span>
              <strong>{snapshot.project.label}</strong>
            </button>
          {/if}

          {#if showSort}
            <label class="filter-field" aria-label={copy.desktop.sort_aria}>
              <span class="filter-label" aria-hidden="true">
                <ArrowUpDown size={14} />
                <span class="filter-label-text">{copy.filters.sort}</span>
              </span>
              <select aria-label={copy.desktop.sort_aria} onchange={setSort}>
                {#each snapshot.sorts as sort}
                  <option value={sort.value} selected={snapshot.sort === sort.value}>{sort.label}</option>
                {/each}
              </select>
            </label>
          {/if}
        </div>
      {/if}

      <div class="page-action-buttons" class:with-filters={showTool || showProject || showSort}>
        <button class="icon-button" type="button" title={copy.actions.refresh_archive} onclick={refresh}>
          <RefreshCw size={16} />
        </button>
        <button class="icon-button" type="button" title={copy.actions.export_current_view} onclick={openReport}>
          <Download size={16} />
        </button>
      </div>
    </div>
  </div>
</header>
