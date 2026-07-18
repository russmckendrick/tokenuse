<script lang="ts">
  import { Focus, RotateCcw, Search } from 'lucide-svelte';
  import { api } from '../api';
  import RelationshipGraph from '../components/RelationshipGraph.svelte';
  import { count } from '../format';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import { graphView, type GraphCamera, type GraphLens } from '../lib/graph.svelte';
  import type { RouteToolId } from '../lib/router.svelte';
  import { staggeredReveal } from '../motion';
  import type {
    DesktopSnapshot,
    GraphEdge,
    GraphMetricId,
    GraphNode,
    GraphNodeKind,
    GraphRelation,
    GraphStats
  } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openProjectPage: (identity: string | null, label: string) => Promise<void>;
  export let openModelPage: (id: string, label: string) => Promise<void>;
  export let openToolPage: (tool: RouteToolId) => void;

  let graphComponent: RelationshipGraph;
  let requestedKey = '';
  let searchQuery = '';
  let searchOpen = false;

  $: state = graphView.state;
  $: desiredKey = [
    snapshot.period,
    snapshot.tool,
    snapshot.project.identity ?? '',
    snapshot.data_generation,
    snapshot.currency,
    state.metric
  ].join('|');
  $: if (desiredKey !== requestedKey) {
    requestedKey = desiredKey;
    void loadGraph(desiredKey, state.metric);
  }

  $: currentData = state.dataKey === desiredKey ? state.data : null;
  $: visibleEdges = currentData ? filterEdges(currentData.edges) : [];
  $: visibleIds = new Set(
    visibleEdges.reduce<string[]>((ids, edge) => {
      ids.push(edge.source, edge.target);
      return ids;
    }, [])
  );
  $: visibleNodes = currentData?.nodes.filter((node) => visibleIds.has(node.id)) ?? [];
  $: selectedNode = state.selectedId
    ? visibleNodes.find((node) => node.id === state.selectedId) ?? null
    : null;
  $: if (state.selectedId && !selectedNode && currentData) state.selectedId = null;
  $: selectedEdges = selectedNode
    ? visibleEdges
        .filter((edge) => edge.source === selectedNode?.id || edge.target === selectedNode?.id)
        .sort((left, right) => metricValue(right.stats) - metricValue(left.stats))
    : [];
  $: strongestSelectedEdge = Math.max(1, ...selectedEdges.map((edge) => metricValue(edge.stats)));
  $: searchResults = searchQuery.trim()
    ? visibleNodes
        .filter((node) => node.label.toLowerCase().includes(searchQuery.trim().toLowerCase()))
        .sort((left, right) => metricValue(right.stats) - metricValue(left.stats))
        .slice(0, 8)
    : [];

  async function loadGraph(key: string, metric: GraphMetricId) {
    const seq = graphView.nextRequest();
    state.loading = true;
    state.error = false;
    try {
      const data = await api.getGraph(metric);
      if (!graphView.isCurrent(seq) || requestedKey !== key) return;
      const previousKey = state.dataKey;
      state.data = data;
      state.dataKey = key;
      state.error = false;
      if (previousKey !== key) state.camera = null;
      if (state.selectedId && !data.nodes.some((node) => node.id === state.selectedId)) {
        state.selectedId = null;
      }
    } catch {
      if (!graphView.isCurrent(seq) || requestedKey !== key) return;
      state.error = true;
    } finally {
      if (graphView.isCurrent(seq)) state.loading = false;
    }
  }

  function filterEdges(edges: GraphEdge[]): GraphEdge[] {
    return edges.filter((edge) => {
      if (state.lens === 'projects') {
        if (edge.relation === 'project_tool' || edge.relation === 'project_model') return true;
        if (state.showCoreTools && edge.relation === 'project_core_tool') return true;
        return state.showMcpServers && edge.relation === 'project_mcp_server';
      }
      if (edge.relation === 'project_tool' || edge.relation === 'tool_model') return true;
      if (state.showCoreTools && edge.relation === 'tool_core_tool') return true;
      return state.showMcpServers && edge.relation === 'tool_mcp_server';
    });
  }

  function metricValue(stats: GraphStats): number {
    if (state.metric === 'spend') return stats.cost_value;
    if (state.metric === 'tokens') return stats.tokens;
    return stats.calls;
  }

  function metricLabel(): string {
    if (state.metric === 'spend') return snapshot.copy.graph.weight_spend;
    if (state.metric === 'tokens') return snapshot.copy.graph.weight_tokens;
    return snapshot.copy.graph.weight_calls;
  }

  function metricDisplay(stats: GraphStats): string {
    if (state.metric === 'spend') return stats.cost;
    return count(state.metric === 'tokens' ? stats.tokens : stats.calls);
  }

  function relationshipStrength(edge: GraphEdge): number {
    return Math.max(2, (metricValue(edge.stats) / strongestSelectedEdge) * 100);
  }

  function saveGraphCamera(next: GraphCamera | null) {
    const current = state.camera;
    if (!current && !next) return;
    if (
      current &&
      next &&
      Math.abs(current.x - next.x) < 0.01 &&
      Math.abs(current.y - next.y) < 0.01 &&
      Math.abs(current.z - next.z) < 0.01 &&
      Math.abs(current.targetX - next.targetX) < 0.01 &&
      Math.abs(current.targetY - next.targetY) < 0.01 &&
      Math.abs(current.targetZ - next.targetZ) < 0.01
    ) {
      return;
    }
    state.camera = next;
  }

  function setLens(lens: GraphLens) {
    if (state.lens === lens) return;
    state.lens = lens;
    state.camera = null;
  }

  function setMetric(metric: GraphMetricId) {
    if (state.metric === metric) return;
    state.metric = metric;
    state.camera = null;
  }

  function toggleCoreTools() {
    state.showCoreTools = !state.showCoreTools;
    state.camera = null;
  }

  function toggleMcpServers() {
    state.showMcpServers = !state.showMcpServers;
    state.camera = null;
  }

  function selectNode(id: string | null) {
    state.selectedId = id;
  }

  function selectSearchResult(node: GraphNode) {
    state.selectedId = node.id;
    searchQuery = node.label;
    searchOpen = false;
    graphComponent?.focus(node.id);
  }

  function neighbor(edge: GraphEdge): GraphNode | null {
    if (!currentData || !selectedNode) return null;
    const id = edge.source === selectedNode.id ? edge.target : edge.source;
    return currentData.nodes.find((node) => node.id === id) ?? null;
  }

  function kindLabel(kind: GraphNodeKind): string {
    if (kind === 'project') return snapshot.copy.graph.kind_project;
    if (kind === 'tool') return snapshot.copy.graph.kind_tool;
    if (kind === 'model') return snapshot.copy.graph.kind_model;
    if (kind === 'core_tool') return snapshot.copy.graph.kind_core_tool;
    return snapshot.copy.graph.kind_mcp_server;
  }

  function relationLabel(relation: GraphRelation): string {
    const copy = snapshot.copy.graph;
    if (relation === 'project_tool') return copy.relation_project_tool;
    if (relation === 'project_model') return copy.relation_project_model;
    if (relation === 'tool_model') return copy.relation_tool_model;
    if (relation === 'project_core_tool') return copy.relation_project_core_tool;
    if (relation === 'tool_core_tool') return copy.relation_tool_core_tool;
    if (relation === 'project_mcp_server') return copy.relation_project_mcp_server;
    return copy.relation_tool_mcp_server;
  }

  function template(text: string, values: Record<string, string>): string {
    return Object.entries(values).reduce(
      (output, [key, value]) => output.split(`{${key}}`).join(value),
      text
    );
  }

  function openSelected() {
    if (!selectedNode) return;
    if (selectedNode.kind === 'project') {
      void openProjectPage(selectedNode.entity_id, selectedNode.label);
    } else if (selectedNode.kind === 'model') {
      void openModelPage(selectedNode.entity_id, selectedNode.label);
    } else if (selectedNode.kind === 'tool') {
      openToolPage(selectedNode.entity_id as RouteToolId);
    }
  }

  function canOpen(node: GraphNode): boolean {
    return node.kind === 'project' || node.kind === 'model' || node.kind === 'tool';
  }

  function handleEscape(event: KeyboardEvent) {
    if (event.key === 'Escape' && state.selectedId) {
      event.stopPropagation();
      state.selectedId = null;
    }
  }
</script>

<svelte:window onkeydown={handleEscape} />

<section
  class="page-flow graph-page"
  use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}
>
  <section class="graph-panel">
    <div class="graph-toolbar">
      <div class="control-group">
        <span>{snapshot.copy.graph.lens_aria}</span>
        <div class="segmented" role="group" aria-label={snapshot.copy.graph.lens_aria}>
          <button
            type="button"
            class:active={state.lens === 'projects'}
            aria-pressed={state.lens === 'projects'}
            onclick={() => setLens('projects')}
          >{snapshot.copy.graph.lens_projects}</button>
          <button
            type="button"
            class:active={state.lens === 'stack'}
            aria-pressed={state.lens === 'stack'}
            onclick={() => setLens('stack')}
          >{snapshot.copy.graph.lens_stack}</button>
        </div>
      </div>

      <div class="control-group">
        <span>{snapshot.copy.graph.metric_aria}</span>
        <div class="segmented" role="group" aria-label={snapshot.copy.graph.metric_aria}>
          <button type="button" class:active={state.metric === 'calls'} aria-pressed={state.metric === 'calls'} onclick={() => setMetric('calls')}>{snapshot.copy.graph.weight_calls}</button>
          <button type="button" class:active={state.metric === 'spend'} aria-pressed={state.metric === 'spend'} onclick={() => setMetric('spend')}>{snapshot.copy.graph.weight_spend}</button>
          <button type="button" class:active={state.metric === 'tokens'} aria-pressed={state.metric === 'tokens'} onclick={() => setMetric('tokens')}>{snapshot.copy.graph.weight_tokens}</button>
        </div>
      </div>

      <div class="layer-controls">
        <button type="button" class:active={state.showCoreTools} aria-pressed={state.showCoreTools} onclick={toggleCoreTools}>{snapshot.copy.graph.core_tools}</button>
        <button type="button" class:active={state.showMcpServers} aria-pressed={state.showMcpServers} onclick={toggleMcpServers}>{snapshot.copy.graph.mcp_servers}</button>
      </div>

      <div class="graph-search">
        <label>
          <span class="sr-only">{snapshot.copy.graph.search_label}</span>
          <Search size={14} aria-hidden="true" />
          <input
            value={searchQuery}
            placeholder={snapshot.copy.graph.search_placeholder}
            onfocus={() => (searchOpen = true)}
            oninput={(event) => {
              searchQuery = (event.currentTarget as HTMLInputElement).value;
              searchOpen = true;
            }}
          />
        </label>
        {#if searchOpen && searchQuery.trim()}
          <div class="search-results">
            {#each searchResults as node (node.id)}
              <button type="button" onmousedown={(event) => event.preventDefault()} onclick={() => selectSearchResult(node)}>
                <span class={`kind-dot kind-${node.kind}`} aria-hidden="true"></span>
                <span>{node.label}</span>
                <small>{kindLabel(node.kind)}</small>
              </button>
            {:else}
              <span>{snapshot.copy.graph.search_no_results}</span>
            {/each}
          </div>
        {/if}
      </div>

      <div class="canvas-actions">
        <button type="button" title={snapshot.copy.graph.fit_view} aria-label={snapshot.copy.graph.fit_view} onclick={() => graphComponent?.fit()}><Focus size={16} /></button>
        <button type="button" title={snapshot.copy.graph.reset_layout} aria-label={snapshot.copy.graph.reset_layout} onclick={() => graphComponent?.reset()}><RotateCcw size={16} /></button>
      </div>
    </div>

    <div class="graph-context">
      <div class="graph-legend" aria-label={snapshot.copy.graph.legend_aria}>
        {#each [
          ['project', snapshot.copy.graph.kind_project],
          ['tool', snapshot.copy.graph.kind_tool],
          ['model', snapshot.copy.graph.kind_model],
          ...(state.showCoreTools ? [['core_tool', snapshot.copy.graph.kind_core_tool]] : []),
          ...(state.showMcpServers ? [['mcp_server', snapshot.copy.graph.kind_mcp_server]] : [])
        ] as item}
          <span><i class={`kind-dot kind-${item[0]}`}></i>{item[1]}</span>
        {/each}
      </div>
      {#if currentData}
        <span class="graph-count mono">
          {template(snapshot.copy.graph.showing, {
            shown: count(visibleNodes.length),
            total: count(currentData.meta.total_nodes),
            edges: count(visibleEdges.length)
          })}
        </span>
      {/if}
    </div>

    {#if currentData && (currentData.meta.shown_nodes < currentData.meta.total_nodes || currentData.meta.shown_edges < currentData.meta.total_edges)}
      <div class="truncation-note">{snapshot.copy.graph.truncated_hint}</div>
    {/if}

    <div class="graph-workspace">
      <div class="canvas-region">
        {#if state.loading && !currentData}
          <div class="graph-state">{snapshot.copy.graph.loading}</div>
        {:else if state.error && !currentData}
          <div class="graph-state error-state">{snapshot.copy.graph.load_error}</div>
        {:else if currentData && visibleNodes.length}
          <RelationshipGraph
            bind:this={graphComponent}
            nodes={visibleNodes}
            edges={visibleEdges}
            lens={state.lens}
            metric={state.metric}
            selectedId={state.selectedId}
            camera={state.camera}
            copy={snapshot.copy.graph}
            {selectNode}
            saveCamera={saveGraphCamera}
          />
        {:else}
          <div class="graph-state">{snapshot.copy.graph.empty}</div>
        {/if}
      </div>

      <aside class="graph-inspector" aria-live="polite">
        {#if selectedNode}
          <div class="inspector-heading">
            <div class="inspector-title">
              {#if selectedNode.kind === 'model'}
                <ProviderIcon id={selectedNode.provider} kind="provider" variant="brand" size={20} />
              {:else if selectedNode.kind === 'tool'}
                <ProviderIcon id={selectedNode.entity_id} kind="tool" size={20} />
              {/if}
              <div><small>{kindLabel(selectedNode.kind)}</small><h2>{selectedNode.label}</h2></div>
            </div>
            {#if canOpen(selectedNode)}
              <button type="button" class="open-detail" onclick={openSelected}>{snapshot.copy.graph.open_details}</button>
            {/if}
          </div>

          <div class="inspector-metric">
            <span>{snapshot.copy.graph.weighted_by} {metricLabel()}</span>
            <strong>{metricDisplay(selectedNode.stats)}</strong>
          </div>

          <dl class="node-stats">
            <div><dt>{snapshot.copy.metrics.calls}</dt><dd>{count(selectedNode.stats.calls)}</dd></div>
            <div><dt>{snapshot.copy.metrics.sessions}</dt><dd>{count(selectedNode.stats.sessions)}</dd></div>
            <div><dt>{snapshot.copy.metrics.tokens}</dt><dd>{count(selectedNode.stats.tokens)}</dd></div>
            <div><dt>{snapshot.copy.metrics.cost}</dt><dd class="money">{selectedNode.stats.cost}</dd></div>
          </dl>
          <div class="last-active"><span>{snapshot.copy.graph.last_active}</span><strong class="mono">{selectedNode.stats.last_activity}</strong></div>

          <h3>{snapshot.copy.graph.relationships}</h3>
          <div class="relationship-list">
            {#each selectedEdges as edge (edge.id)}
              {@const related = neighbor(edge)}
              {#if related}
                <button type="button" onclick={() => { selectNode(related.id); graphComponent?.focus(related.id); }}>
                  <i class="relationship-rank" style={`width: ${relationshipStrength(edge)}%`}></i>
                  <span class={`kind-dot kind-${related.kind}`} aria-hidden="true"></span>
                  <span class="relationship-name"><strong>{related.label}</strong><small>{relationLabel(edge.relation)}</small></span>
                  <span class="relationship-value mono">
                    {state.metric === 'spend' ? edge.stats.cost : count(state.metric === 'tokens' ? edge.stats.tokens : edge.stats.calls)}
                  </span>
                </button>
              {/if}
            {/each}
          </div>
        {:else}
          <div class="inspector-empty">
            <h2>{snapshot.copy.graph.selection_empty_title}</h2>
            <p>{snapshot.copy.graph.selection_empty_detail}</p>
            <p class="inspector-hint">{snapshot.copy.graph.selection_empty_hint}</p>
            <dl class="empty-summary">
              <div><dt>{snapshot.copy.graph.visible_entities}</dt><dd>{count(visibleNodes.length)}</dd></div>
              <div><dt>{snapshot.copy.graph.visible_links}</dt><dd>{count(visibleEdges.length)}</dd></div>
              <div><dt>{snapshot.copy.graph.weighted_by}</dt><dd>{metricLabel()}</dd></div>
            </dl>
          </div>
        {/if}
      </aside>
    </div>
  </section>
</section>

<style>
  .graph-page {
    flex: 1 0 auto;
    min-height: 0;
  }

  .graph-panel {
    display: flex;
    flex: 1 0 auto;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-neutral);
    overflow: hidden;
  }

  .graph-toolbar {
    min-height: 64px;
    display: flex;
    align-items: flex-end;
    gap: var(--space-lg);
    padding: var(--space-lg);
    border-bottom: 1px solid var(--color-border-soft);
  }

  .control-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
  }

  .control-group > span {
    color: var(--color-muted-2);
    font-size: var(--text-label);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .layer-controls {
    display: flex;
    gap: var(--space-sm);
  }

  .layer-controls button,
  .canvas-actions button {
    min-height: 30px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-muted);
    font: inherit;
  }

  .layer-controls button {
    padding: 0 var(--space-md);
  }

  .layer-controls button:hover,
  .layer-controls button.active,
  .canvas-actions button:hover {
    color: var(--color-on-surface);
    border-color: var(--color-muted-2);
    background: var(--color-neutral-hover);
  }

  .layer-controls button.active {
    box-shadow: inset 2px 0 var(--color-primary);
  }

  .graph-search {
    position: relative;
    flex: 1;
    min-width: 170px;
    max-width: 320px;
    margin-left: auto;
  }

  .graph-search label {
    min-height: 32px;
    display: flex;
    align-items: center;
    gap: var(--space-md);
    padding: 0 var(--space-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-muted-2);
  }

  .graph-search input {
    min-width: 0;
    width: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--color-on-surface);
    font: inherit;
  }

  .search-results {
    position: absolute;
    z-index: 8;
    top: calc(100% + 4px);
    right: 0;
    left: 0;
    display: flex;
    flex-direction: column;
    padding: var(--space-sm);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-neutral);
    box-shadow: var(--elevation-popover);
  }

  .search-results > span {
    padding: var(--space-md);
    color: var(--color-muted-2);
    font-size: 12px;
  }

  .search-results button {
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-md);
    min-height: 30px;
    padding: 0 var(--space-md);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-on-surface);
    text-align: left;
  }

  .search-results button:hover,
  .search-results button:focus-visible {
    background: var(--color-neutral-hover);
  }

  .search-results small {
    color: var(--color-muted-2);
  }

  .canvas-actions {
    display: flex;
    gap: var(--space-sm);
  }

  .canvas-actions button {
    width: 32px;
    display: grid;
    place-items: center;
  }

  .graph-context {
    min-height: 38px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-lg);
    padding: var(--space-md) var(--space-lg);
    border-bottom: 1px solid var(--color-border-soft);
  }

  .graph-legend {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-lg);
  }

  .graph-legend span {
    display: inline-flex;
    align-items: center;
    gap: var(--space-sm);
    color: var(--color-muted);
    font-size: 11px;
  }

  .kind-dot {
    width: 7px;
    height: 7px;
    display: inline-block;
    border-radius: 2px;
    background: var(--color-muted);
  }

  .kind-project { background: var(--color-tertiary); }
  .kind-tool { background: var(--color-primary); }
  .kind-model { background: var(--color-magenta); }
  .kind-core_tool { background: var(--color-cyan); }
  .kind-mcp_server { background: var(--color-secondary); }

  .graph-count {
    color: var(--color-muted-2);
    font-size: 11px;
    text-align: right;
  }

  .truncation-note {
    padding: var(--space-sm) var(--space-lg);
    border-bottom: 1px solid var(--color-border-soft);
    background: color-mix(in srgb, var(--color-warning) 5%, transparent);
    color: var(--color-muted);
    font-size: 11px;
  }

  .graph-workspace {
    display: grid;
    flex: 1 1 0;
    grid-template-columns: minmax(0, 1fr) 320px;
    min-width: 0;
    min-height: 540px;
  }

  .canvas-region {
    display: flex;
    min-width: 0;
    min-height: 0;
    background: var(--color-surface-sunken);
  }

  .graph-state {
    flex: 1 1 auto;
    min-height: 540px;
    display: grid;
    place-items: center;
    padding: var(--space-2xl);
    color: var(--color-muted-2);
  }

  .error-state {
    color: var(--color-error);
  }

  .graph-inspector {
    min-width: 0;
    min-height: 0;
    padding: var(--space-xl);
    border-left: 1px solid var(--color-border);
    background: var(--color-neutral);
    overflow-y: auto;
  }

  .inspector-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-md);
    padding-bottom: var(--space-lg);
    border-bottom: 1px solid var(--color-border-soft);
  }

  .inspector-title {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-md);
  }

  .inspector-title div {
    min-width: 0;
  }

  .inspector-title small,
  .graph-inspector h3,
  .node-stats dt,
  .last-active span {
    color: var(--color-muted-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .inspector-title h2,
  .inspector-empty h2 {
    margin: 2px 0 0;
    color: var(--color-on-surface);
    font-size: 15px;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }

  .open-detail {
    flex: 0 0 auto;
    min-height: 28px;
    padding: 0 var(--space-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-on-surface);
    font: inherit;
    font-size: 11px;
  }

  .open-detail:hover {
    background: var(--color-neutral-hover);
  }

  .node-stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    margin: var(--space-lg) 0 0;
  }

  .inspector-metric {
    padding: var(--space-lg) 0;
    border-bottom: 1px solid var(--color-border-soft);
  }

  .inspector-metric span {
    display: block;
    color: var(--color-muted-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .inspector-metric strong {
    display: block;
    margin-top: var(--space-sm);
    color: var(--color-primary);
    font-family: var(--font-mono);
    font-size: 22px;
    font-variant-numeric: tabular-nums;
  }

  .node-stats div {
    padding: var(--space-md) 0;
    border-bottom: 1px solid var(--color-border-soft);
  }

  .node-stats div:nth-child(odd) {
    padding-right: var(--space-md);
    border-right: 1px solid var(--color-border-soft);
  }

  .node-stats div:nth-child(even) {
    padding-left: var(--space-md);
  }

  .node-stats dd {
    margin: var(--space-sm) 0 0;
    color: var(--color-on-surface);
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .node-stats dd.money {
    color: var(--color-warning);
  }

  .last-active {
    display: flex;
    justify-content: space-between;
    gap: var(--space-md);
    padding: var(--space-lg) 0;
    border-bottom: 1px solid var(--color-border-soft);
  }

  .last-active strong {
    color: var(--color-muted);
    font-size: 11px;
  }

  .graph-inspector h3 {
    margin: var(--space-xl) 0 var(--space-md);
  }

  .relationship-list {
    display: flex;
    flex-direction: column;
  }

  .relationship-list button {
    position: relative;
    min-height: 42px;
    display: grid;
    grid-template-columns: 8px minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-md);
    padding: var(--space-sm) 0;
    border: 0;
    border-bottom: 1px solid var(--color-border-soft);
    background: transparent;
    color: var(--color-on-surface);
    text-align: left;
  }

  .relationship-rank {
    position: absolute;
    right: auto;
    bottom: 0;
    left: 0;
    height: 1px;
    background: var(--color-primary);
    opacity: 0.62;
    pointer-events: none;
  }

  .relationship-list button:hover,
  .relationship-list button:focus-visible {
    background: var(--color-neutral-hover);
  }

  .relationship-name {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .relationship-name strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }

  .relationship-name small {
    color: var(--color-muted-2);
    font-size: 10px;
  }

  .relationship-value {
    color: var(--color-muted);
    font-size: 11px;
  }

  .inspector-empty {
    min-height: 240px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .inspector-empty p {
    margin: var(--space-md) 0 0;
    color: var(--color-muted);
    line-height: 1.55;
  }

  .inspector-empty .inspector-hint {
    color: var(--color-muted-2);
    font-size: 11px;
  }

  .empty-summary {
    width: 100%;
    margin: var(--space-xl) 0 0;
    border-top: 1px solid var(--color-border-soft);
  }

  .empty-summary div {
    min-height: 34px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-lg);
    border-bottom: 1px solid var(--color-border-soft);
  }

  .empty-summary dt {
    color: var(--color-muted-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .empty-summary dd {
    margin: 0;
    color: var(--color-on-surface);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  @media (max-width: 1120px) {
    .graph-toolbar {
      flex-wrap: wrap;
    }

    .graph-search {
      order: 5;
      max-width: none;
      flex-basis: 100%;
    }
  }

  @media (max-width: 820px) {
    .graph-workspace {
      flex: 0 0 auto;
      grid-template-columns: minmax(0, 1fr);
    }

    .graph-inspector {
      border-top: 1px solid var(--color-border);
      border-left: 0;
      overflow-y: visible;
    }

    .graph-context {
      align-items: flex-start;
      flex-direction: column;
    }

    .graph-count {
      text-align: left;
    }

    .graph-state {
      min-height: 520px;
    }
  }
</style>
