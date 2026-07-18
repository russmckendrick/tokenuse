<script lang="ts">
  import { ArrowLeft } from 'lucide-svelte';
  import { api } from '../api';
  import Donut from '../charts/Donut.svelte';
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import ActivityCategoryTable from '../components/tables/ActivityCategoryTable.svelte';
  import ProjectTable from '../components/tables/ProjectTable.svelte';
  import { count, rankLabel, rankPercent } from '../format';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, ModelPageData, ShareMetric } from '../types';

  export let snapshot: DesktopSnapshot;
  export let model: { id: string; label: string };
  export let openSession: (key: string) => void;
  export let openProject: (name: string) => void;
  export let backLabel: string;
  export let goBack: () => void;

  const SESSION_PAGE_SIZE = 60;

  let data: ModelPageData | null = null;
  let pageKey = '';
  let visibleSessions = SESSION_PAGE_SIZE;

  $: {
    const key = [
      model.id,
      snapshot.period,
      snapshot.sort,
      snapshot.data_generation,
      snapshot.currency
    ].join('|');
    if (key !== pageKey) {
      pageKey = key;
      void loadPage();
    }
  }

  async function loadPage() {
    try {
      const next = await api.getModelPage(model.id);
      if (next.canonical_id === model.id) {
        data = next;
        visibleSessions = SESSION_PAGE_SIZE;
      }
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  $: sessions = data?.sessions ?? [];
  $: shownSessions = sessions.slice(0, visibleSessions);

  /** Sentinel row action: grow the rendered session window as it scrolls
   * into view, so long histories never render in one go. */
  function loadMoreSentinel(node: HTMLElement) {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        visibleSessions += SESSION_PAGE_SIZE;
      }
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function handleSessionRowKey(event: KeyboardEvent, key: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openSession(key);
    }
  }

  /** Per-tool donut shares come from call counts — the split's costs are
   * display strings, calls are the numeric signal it carries. */
  $: splitRows = ((): ShareMetric[] => {
    const rows = data?.per_tool ?? [];
    const total = rows.reduce((sum, row) => sum + row.calls, 0);
    return rows.map((row) => ({
      key: row.tool,
      label: row.tool_label,
      cost: row.cost,
      calls: row.calls,
      share: total > 0 ? Math.round((row.calls / total) * 1000) : 0
    }));
  })();

  type CompositionRow = { label: string; amount: string; share: number };

  $: compositionRows = ((): CompositionRow[] => {
    const composition = data?.detail?.composition;
    if (!composition) return [];
    const rows = [
      { label: snapshot.copy.metrics.input, amount: composition.input_label, raw: composition.input },
      { label: snapshot.copy.metrics.output, amount: composition.output_label, raw: composition.output },
      { label: snapshot.copy.metrics.cached, amount: composition.cache_read_label, raw: composition.cache_read },
      { label: snapshot.copy.metrics.written, amount: composition.cache_write_label, raw: composition.cache_write }
    ];
    const max = Math.max(...rows.map((row) => row.raw), 1);
    return rows.map((row) => ({
      label: row.label,
      amount: row.amount,
      share: Math.round((row.raw / max) * 100)
    }));
  })();

  type PricingRow = { label: string; value: string; note: string };

  $: pricingRows = ((): PricingRow[] => {
    const pricing = data?.detail?.pricing;
    if (!pricing) return [];
    const perMtok = snapshot.copy.metrics.per_mtok;
    return [
      { label: snapshot.copy.metrics.input_price, value: pricing.input_per_mtok, note: perMtok },
      { label: snapshot.copy.metrics.output_price, value: pricing.output_per_mtok, note: perMtok },
      {
        label: snapshot.copy.metrics.cache_read_price,
        value: pricing.cache_read_per_mtok,
        note: pricing.cache_read_rate === '-' ? perMtok : `${perMtok} · ${pricing.cache_read_rate}`
      },
      {
        label: snapshot.copy.metrics.cache_write_price,
        value: pricing.cache_write_per_mtok,
        note: pricing.cache_write_rate === '-' ? perMtok : `${perMtok} · ${pricing.cache_write_rate}`
      },
      {
        label: snapshot.copy.metrics.avg_cost_per_call,
        value: pricing.avg_cost_per_call,
        note: snapshot.currency
      }
    ];
  })();
</script>

{#if data}
  <section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
    <div class="drill-back">
      <button type="button" onclick={goBack}><ArrowLeft size={15} /> {backLabel}</button>
    </div>

    <section class="kpis hero-kpis model-kpis">
      <div>
        <span>{snapshot.copy.metrics.cost}</span>
        <strong class="hero-value" use:countUp={data.dashboard.summary.cost}>{data.dashboard.summary.cost}</strong>
        <small>{snapshot.currency}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.calls}</span>
        <strong use:countUp={data.dashboard.summary.calls}>{data.dashboard.summary.calls}</strong>
        <small>{snapshot.copy.metrics.in} {data.dashboard.summary.input}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.sessions}</span>
        <strong use:countUp={data.dashboard.summary.sessions}>{data.dashboard.summary.sessions}</strong>
        <small>{snapshot.copy.metrics.active_set}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.cache_hit}</span>
        <strong use:countUp={data.dashboard.summary.cache_hit}>{data.dashboard.summary.cache_hit}</strong>
        <small>{snapshot.copy.metrics.cached} {data.dashboard.summary.cached}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.avg_cost_per_call}</span>
        <strong>{data.detail?.pricing.avg_cost_per_call ?? snapshot.copy.tables.blank}</strong>
        <small>{snapshot.currency}</small>
      </div>
      <div>
        <span>{snapshot.copy.metrics.output}</span>
        <strong use:countUp={data.dashboard.summary.output}>{data.dashboard.summary.output}</strong>
        <small>{snapshot.copy.metrics.tokens}</small>
      </div>
    </section>

    <Panel title={snapshot.copy.panels.activity_pulse} tone="blue">
      <ActivityPulse points={data.dashboard.activity_timeline} copy={snapshot.copy} />
    </Panel>

    <section class="duo-grid">
      <Panel title={snapshot.copy.metrics.sessions} tone="red" scrollable>
        <table class="data-table session-table">
          <thead>
            <tr>
              <th>{snapshot.copy.tables.date}</th>
              <th>{snapshot.copy.tables.project}</th>
              <th>{snapshot.copy.tables.cost}</th>
              <th>{snapshot.copy.tables.calls}</th>
            </tr>
          </thead>
          <tbody>
            {#each shownSessions as session (session.key)}
              <tr
                class="rank-row click-row"
                style:--rank-fill={`${rankPercent(session.value)}%`}
                title={rankLabel(snapshot.copy.timeline.relative_rank, session.value)}
                tabindex="0"
                onclick={() => openSession(session.key)}
                onkeydown={(event) => handleSessionRowKey(event, session.key)}
              >
                <td>{session.date}</td>
                <td class="muted-cell">{session.project}</td>
                <td class="money">{session.cost}</td>
                <td>{count(session.calls)}</td>
              </tr>
            {:else}
              <tr><td colspan="4" class="empty-cell">{snapshot.copy.empty.no_sessions}</td></tr>
            {/each}
            {#if visibleSessions < sessions.length}
              <tr use:loadMoreSentinel><td colspan="4" class="empty-cell">{count(sessions.length - visibleSessions)}+</td></tr>
            {/if}
          </tbody>
        </table>
      </Panel>

      <Panel title={snapshot.copy.desktop.token_composition} tone="cyan" scrollable>
        <table class="data-table composition-table">
          <thead><tr><th>{snapshot.copy.tables.name}</th><th>{snapshot.copy.metrics.tokens}</th></tr></thead>
          <tbody>
            {#each compositionRows as row (row.label)}
              <tr class="rank-row" style:--rank-fill={`${row.share}%`}>
                <td>{row.label}</td>
                <td class="mono">{row.amount}</td>
              </tr>
            {:else}
              <tr><td colspan="2" class="empty-cell">{snapshot.copy.empty.no_data}</td></tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.desktop.per_tool_split} tone="yellow">
        {#if data.per_tool.length === 1}
          {@const only = data.per_tool[0]}
          <div class="single-tool">
            <div class="single-tool-head">
              {#if only.tool}
                <ProviderIcon id={only.tool} kind="tool" size={20} />
              {/if}
              <strong>{only.tool_label}</strong>
            </div>
            <div class="single-tool-stats">
              <span><small>{snapshot.copy.metrics.cost}</small><strong class="mono money">{only.cost}</strong></span>
              <span><small>{snapshot.copy.metrics.calls}</small><strong class="mono">{count(only.calls)}</strong></span>
            </div>
          </div>
        {:else}
          <Donut
            rows={splitRows}
            colorBy="tool"
            centerLabel={snapshot.copy.metrics.calls}
            ariaLabel={snapshot.copy.desktop.per_tool_split}
            emptyLabel={snapshot.copy.empty.no_data}
          />
        {/if}
      </Panel>

      <Panel title={snapshot.copy.desktop.pricing} tone="green" scrollable>
        {#if data.detail}
          <table class="data-table pricing-table">
            <thead><tr><th>{snapshot.copy.tables.name}</th><th>{snapshot.copy.metrics.cost}</th><th></th></tr></thead>
            <tbody>
              {#each pricingRows as row (row.label)}
                <tr>
                  <td>{row.label}</td>
                  <td class="money">{row.value}</td>
                  <td class="muted-cell pricing-note">{row.note}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          {#if data.detail.pricing.fallback}
            <div class="pricing-fallback">{snapshot.copy.desktop.pricing_fallback_note}</div>
          {/if}
        {:else}
          <div class="empty-state">{snapshot.copy.empty.no_data}</div>
        {/if}
      </Panel>
    </section>

    <section class="duo-grid">
      <Panel title={snapshot.copy.panels.by_project} tone="magenta" scrollable>
        <ProjectTable rows={data.dashboard.projects} copy={snapshot.copy} {openProject} />
      </Panel>
      <Panel title={snapshot.copy.categories.heading} tone="cyan" scrollable>
        <ActivityCategoryTable rows={data.dashboard.by_activity} copy={snapshot.copy} />
      </Panel>
    </section>
  </section>
{/if}

<style>
  .drill-back {
    display: flex;
  }

  .model-kpis {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .single-tool {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 6px 2px;
  }

  .single-tool-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-body);
  }

  .single-tool-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: 10px;
  }

  .single-tool-stats span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .single-tool-stats small {
    color: var(--color-muted-2);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: var(--text-label);
  }

  .single-tool-stats strong {
    font-size: var(--text-display);
    font-weight: 600;
  }

  .pricing-note {
    font-size: 11px;
  }

  .pricing-fallback {
    padding: 8px 2px 2px;
    font-size: 12px;
    color: var(--color-warning);
  }
</style>
