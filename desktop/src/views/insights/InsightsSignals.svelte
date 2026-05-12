<script lang="ts">
  import { BarChart3, ChevronRight, CircleDot } from 'lucide-svelte';
  import { fadeIn } from '../../motion';
  import type { DesktopSnapshot } from '../../types';
  import {
    barStyle,
    copyTemplate,
    normalizeSeverity,
    type InsightModel,
    type InsightRow
  } from './model';

  export let snapshot: DesktopSnapshot;
  export let model: InsightModel;
  export let openDetail: (row: InsightRow, from: 'signals') => void;
</script>

<section class="screen signals-screen" use:fadeIn>
  <div class="screen-head">
    <div>
      <span class="eyebrow">{model.copy.screen_signals}</span>
      <h3>{model.copy.signals_title}</h3>
      <p>{model.copy.signals_subtitle}</p>
    </div>
  </div>

  <div class="signals-layout">
    <section class="panel signal-groups">
      {#if model.signalGroups.length}
        {#each model.signalGroups as group (group.id)}
          <section class="signal-group">
            <div class="group-head">
              <h4>{group.label}</h4>
              <span>{copyTemplate(model.copy.run_items, { count: String(group.rows.length) })}</span>
            </div>
            <div class="signal-list">
              {#each group.rows as row (row.id)}
                <button
                  type="button"
                  class={`signal-row severity-${row.severity}`}
                  onclick={() => openDetail(row, 'signals')}
                >
                  <span class="row-rail"></span>
                  <span class="row-body">
                    <span class="row-meta">
                      <strong class={`severity-label ${row.severity}`}>{row.severityLabel}</strong>
                      <span>{row.scopeLabel}</span>
                    </span>
                    <strong>{row.title}</strong>
                    <span>{row.impact ?? row.body}</span>
                  </span>
                  <ChevronRight size={15} />
                </button>
              {/each}
            </div>
          </section>
        {/each}
      {:else}
        <div class="empty-state">
          <CircleDot size={18} />
          <strong>{model.copy.signals_empty_title}</strong>
          <p>{model.copy.signals_empty_detail}</p>
        </div>
      {/if}
    </section>

    <aside class="panel signal-map-panel">
      <div class="panel-head compact">
        <span class="panel-kicker"><BarChart3 size={14} /> {model.copy.signal_map_title}</span>
      </div>
      <div class="bar-list">
        {#each snapshot.insights.summary.by_severity as severity}
          <div class={`bar-row severity-${normalizeSeverity(severity.id)}`} style={barStyle(severity.count, model.maxSeverityCount)}>
            <span>{severity.label}</span>
            <strong>{severity.count}</strong>
            <i></i>
          </div>
        {/each}
        {#each snapshot.insights.summary.by_category as category}
          <div class="bar-row category" style={barStyle(category.count, model.maxCategoryCount)}>
            <span>{category.label}</span>
            <strong>{category.count}</strong>
            <i></i>
          </div>
        {/each}
      </div>
    </aside>
  </div>
</section>
