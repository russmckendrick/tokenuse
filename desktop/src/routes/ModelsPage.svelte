<script lang="ts">
  import { ChevronRight } from 'lucide-svelte';
  import { api } from '../api';
  import RankBar from '../components/RankBar.svelte';
  import { count } from '../format';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, ModelCatalogEntry } from '../types';

  export let snapshot: DesktopSnapshot;

  type ProviderGroup = {
    provider: string;
    label: string;
    entries: ModelCatalogEntry[];
  };

  let catalog: ModelCatalogEntry[] = [];
  let catalogKey = '';
  let expanded: Record<string, boolean> = {};

  $: {
    const key = [snapshot.period, snapshot.data_generation, snapshot.currency].join('|');
    if (key !== catalogKey) {
      catalogKey = key;
      void loadCatalog();
    }
  }

  async function loadCatalog() {
    try {
      catalog = await api.getModelCatalog(snapshot.period);
    } catch {
      // Keep the previous catalog on transient errors.
    }
  }

  function groups(entries: ModelCatalogEntry[]): ProviderGroup[] {
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

  $: providerGroups = groups(catalog);
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
  {#each providerGroups as group}
    <Panel title={group.label} tone="magenta">
      <svelte:fragment slot="title-icon">
        <ProviderIcon id={group.provider} size={20} variant="brand" />
      </svelte:fragment>
      <table class="data-table catalog-table">
        <thead>
          <tr>
            <th></th>
            <th>{snapshot.copy.tables.model}</th>
            <th>{snapshot.copy.tables.cost}</th>
            <th>{snapshot.copy.tables.calls}</th>
            <th>{snapshot.copy.metrics.tokens}</th>
            <th>{snapshot.copy.tables.cache}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each group.entries as entry}
            <tr
              class="catalog-row"
              class:expanded={expanded[entry.canonical_id]}
              onclick={() => toggle(entry.canonical_id)}
            >
              <td><RankBar value={entry.value} ariaLabel={`${entry.name} ${snapshot.copy.desktop.rank}`} /></td>
              <td>
                <span class="catalog-model">
                  <ProviderIcon id={group.provider} size={14} />
                  {entry.name}
                  <small class="muted-cell">{entry.family}</small>
                </span>
              </td>
              <td class="money">{entry.cost}</td>
              <td>{count(entry.calls)}</td>
              <td class="mono">{entry.tokens}</td>
              <td>{entry.cache_hit}</td>
              <td class="expander" aria-hidden="true">
                <ChevronRight size={13} />
              </td>
            </tr>
            {#if expanded[entry.canonical_id]}
              <tr class="catalog-split">
                <td></td>
                <td colspan="6">
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
        </tbody>
      </table>
    </Panel>
  {/each}
</section>

<style>
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

  .expander {
    width: 20px;
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
