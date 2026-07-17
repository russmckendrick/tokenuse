<script lang="ts">
  import { ChevronRight } from 'lucide-svelte';
  import { api } from '../api';
  import { count, rankLabel, rankPercent } from '../format';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, ModelCatalogEntry, PeriodId } from '../types';

  type CatalogRow = ModelCatalogEntry & {
    ranges: Partial<Record<PeriodId, ModelCatalogEntry>>;
  };

  export let snapshot: DesktopSnapshot;

  type ProviderGroup = {
    provider: string;
    label: string;
    entries: CatalogRow[];
  };

  let catalog: CatalogRow[] = [];
  let catalogKey = '';
  let expanded: Record<string, boolean> = {};

  $: {
    const key = [snapshot.period, snapshot.data_generation, snapshot.currency].join('|');
    if (key !== catalogKey) {
      catalogKey = key;
      void loadCatalog(key, snapshot.period);
    }
  }

  async function loadCatalog(key: string, selectedPeriod: PeriodId) {
    try {
      const catalogs = await Promise.all(
        snapshot.periods.map(async (period) => ({
          period: period.value,
          entries: await api.getModelCatalog(period.value)
        }))
      );
      const allTime = catalogs.find((catalog) => catalog.period === 'all-time')?.entries ?? [];
      const selected = catalogs.find((periodCatalog) => periodCatalog.period === selectedPeriod)?.entries ?? [];
      const selectedIds = new Set(selected.map((entry) => entry.canonical_id));
      const orderedEntries = [
        ...selected,
        ...allTime.filter((entry) => !selectedIds.has(entry.canonical_id))
      ];
      const nextCatalog = orderedEntries.map((entry) => {
        const ranges: Partial<Record<PeriodId, ModelCatalogEntry>> = {};
        for (const periodCatalog of catalogs) {
          const rangeEntry = periodCatalog.entries.find(
            (candidate) => candidate.canonical_id === entry.canonical_id
          );
          if (rangeEntry) ranges[periodCatalog.period] = rangeEntry;
        }
        const selectedEntry = ranges[selectedPeriod];
        return {
          ...entry,
          cost: selectedEntry?.cost ?? '-',
          calls: selectedEntry?.calls ?? 0,
          tokens: selectedEntry?.tokens ?? '-',
          cache_hit: selectedEntry?.cache_hit ?? '-',
          value: selectedEntry?.value ?? 0,
          per_tool: selectedEntry?.per_tool ?? [],
          ranges
        };
      });
      if (catalogKey === key) catalog = nextCatalog;
    } catch {
      // Keep the previous catalog on transient errors.
    }
  }

  function groups(entries: CatalogRow[]): ProviderGroup[] {
    const grouped: ProviderGroup[] = [];
    for (const entry of entries) {
      let group = grouped.find((g) => g.provider === entry.provider);
      if (!group) {
        group = { provider: entry.provider, label: entry.provider_label, entries: [] };
        grouped.push(group);
      }
      group.entries.push(entry);
    }
    return grouped;
  }

  function toggle(id: string) {
    expanded = { ...expanded, [id]: !expanded[id] };
  }

  function handleRowKey(event: KeyboardEvent, id: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      toggle(id);
    }
  }

  $: providerGroups = groups(catalog);
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
  {#each providerGroups as group}
    <Panel title={group.label} tone="magenta" scrollable>
      <svelte:fragment slot="title-icon">
        <ProviderIcon id={group.provider} size={20} variant="brand" />
      </svelte:fragment>
      <table class="data-table catalog-table">
        <colgroup>
          <col class="model-column" />
          {#each snapshot.periods as _}<col class="range-column" />{/each}
          <col class="cache-column" />
          <col class="expander-column" />
        </colgroup>
        <thead>
          <tr>
            <th>{snapshot.copy.tables.model}</th>
            {#each snapshot.periods as period}
              <th class="range-heading">{period.label}</th>
            {/each}
            <th>{snapshot.copy.tables.cache}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each group.entries as entry}
            <tr
              class="catalog-row rank-row"
              class:expanded={expanded[entry.canonical_id]}
              style:--rank-fill={`${rankPercent(entry.value)}%`}
              title={rankLabel(snapshot.copy.timeline.relative_rank, entry.value)}
              tabindex="0"
              aria-expanded={expanded[entry.canonical_id] ?? false}
              onclick={() => toggle(entry.canonical_id)}
              onkeydown={(event) => handleRowKey(event, entry.canonical_id)}
            >
              <td>
                <span class="catalog-model">
                  <ProviderIcon id={group.provider} size={14} />
                  {entry.name}
                  <small class="muted-cell">{entry.family}</small>
                  <span class="sr-only">{rankLabel(snapshot.copy.timeline.relative_rank, entry.value)}</span>
                </span>
              </td>
              {#each snapshot.periods as period}
                <td class="range-cell">
                  {#if entry.ranges[period.value]}
                    <strong class="money">{entry.ranges[period.value]?.cost}</strong>
                    <small>{count(entry.ranges[period.value]?.calls ?? 0)} {snapshot.copy.metrics.calls}</small>
                  {:else}
                    <span class="muted-cell">-</span>
                  {/if}
                </td>
              {/each}
              <td>{entry.cache_hit}</td>
              <td class="expander" aria-hidden="true">
                <ChevronRight size={13} />
              </td>
            </tr>
            {#if expanded[entry.canonical_id]}
              <tr class="catalog-split">
                <td colspan="8">
                  <div class="split-rows">
                    <span class="split-title">{snapshot.copy.desktop.per_tool_split}</span>
                    {#each entry.per_tool as split}
                      <span class="split-row">
                        <ProviderIcon id={split.tool} kind="tool" size={13} />
                        {split.tool_label}
                        <em class="mono money">{split.cost}</em>
                        <em class="mono">{count(split.calls)} {snapshot.copy.metrics.calls}</em>
                      </span>
                    {/each}
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
          {#if group.entries.length === 0}
            <tr><td colspan="8" class="empty-cell">{snapshot.copy.empty.no_models}</td></tr>
          {/if}
        </tbody>
      </table>
    </Panel>
  {:else}
    <div class="empty-state">{snapshot.copy.empty.no_models}</div>
  {/each}
</section>

<style>
  .catalog-table {
    width: 100%;
    table-layout: fixed;
  }

  .model-column {
    width: auto;
  }

  .range-column {
    width: 104px;
  }

  .cache-column {
    width: 72px;
  }

  .expander-column {
    width: 24px;
  }

  .catalog-row {
    cursor: pointer;
  }

  .catalog-model {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }

  .catalog-model small {
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .range-heading {
    width: 104px;
  }

  .range-cell {
    width: 104px;
    white-space: nowrap;
  }

  .range-cell strong,
  .range-cell small {
    display: block;
  }

  .range-cell small {
    margin-top: 2px;
    font-size: 10px;
    font-family: var(--font-ui);
  }

  .expander {
    width: 24px;
    padding-left: 0;
    padding-right: 0;
    text-align: center;
    color: var(--color-muted-2);
  }

  .catalog-row.expanded .expander :global(svg) {
    transform: rotate(90deg);
  }

  .expander :global(svg) {
    transition: transform var(--motion-fast) var(--ease-standard);
  }

  .catalog-split td {
    padding-top: 0;
  }

  .split-rows {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 4px 0 8px;
  }

  .split-title {
    font-family: var(--font-ui);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-muted-2);
  }

  .split-row {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--color-muted);
  }

  .split-row em {
    font-style: normal;
    color: var(--color-on-surface);
  }
</style>
