<script lang="ts">
  import { onMount } from 'svelte';
  import { Search } from 'lucide-svelte';
  import { api } from '../api';
  import { count, rankPercent } from '../format';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import { scrollback } from '../lib/scrollback.svelte';
  import type { DesktopSnapshot, ScrollbackSessionResult } from '../types';

  export let snapshot: DesktopSnapshot;
  export let openSession: (key: string) => void;

  const state = scrollback.state;

  const DEBOUNCE_MS = 300;
  const MIN_QUERY_CHARS = 2;

  let inputEl: HTMLInputElement | null = null;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    if (!state.results) {
      inputEl?.focus();
    }
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  });

  function fill(template: string, values: Record<string, string>): string {
    return template.replace(/\{(\w+)\}/g, (raw, key: string) => values[key] ?? raw);
  }

  function scheduleSearch() {
    if (debounceTimer) clearTimeout(debounceTimer);
    const trimmed = state.query.trim();
    if (trimmed.length < MIN_QUERY_CHARS) {
      // Below the threshold nothing should land later: invalidate any
      // in-flight search, and clear results once the query is gone.
      scrollback.nextRequest();
      state.searching = false;
      if (!trimmed) {
        state.results = null;
        state.error = null;
      }
      return;
    }
    debounceTimer = setTimeout(() => void runSearch(), DEBOUNCE_MS);
  }

  async function runSearch() {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    const query = state.query.trim();
    if (!query) {
      scrollback.nextRequest();
      state.results = null;
      state.error = null;
      state.searching = false;
      return;
    }
    const seq = scrollback.nextRequest();
    state.searching = true;
    try {
      const results = await api.searchTranscripts({
        query,
        tool: state.tool || null,
        project: state.project || null
      });
      if (!scrollback.isCurrent(seq)) return;
      state.results = results;
      state.error = null;
    } catch (err) {
      if (!scrollback.isCurrent(seq)) return;
      state.error = fill(snapshot.copy.scrollback.error, {
        error: err instanceof Error ? err.message : String(err)
      });
    } finally {
      if (scrollback.isCurrent(seq)) {
        state.searching = false;
      }
    }
  }

  function handleInputKey(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void runSearch();
    }
  }

  function handleRowKey(event: KeyboardEvent, session: ScrollbackSessionResult) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      openSession(session.key);
    }
  }

  function toolLabel(id: string): string {
    return snapshot.tools.find((tool) => tool.value === id)?.label ?? id;
  }

  function dateOf(session: ScrollbackSessionResult): string {
    return session.last_timestamp?.slice(0, 10) ?? '';
  }

  /** Rank wash relative to the busiest session in this result set. */
  function matchRank(session: ScrollbackSessionResult): number {
    const max = state.results?.sessions.reduce((acc, s) => Math.max(acc, s.match_count), 0) ?? 0;
    return max > 0 ? rankPercent((session.match_count / max) * 100) : 0;
  }

  function extraMatches(session: ScrollbackSessionResult): number {
    return Math.max(0, session.match_count - session.matches.length);
  }
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.03 }}>
  <div class="scrollback-toolbar">
    <label class="filter-field scrollback-query" aria-label={snapshot.copy.scrollback.input_label}>
      <Search size={14} />
      <input
        bind:this={inputEl}
        bind:value={state.query}
        placeholder={snapshot.copy.scrollback.input_placeholder}
        oninput={scheduleSearch}
        onkeydown={handleInputKey}
      />
    </label>
    <label class="filter-field" aria-label={snapshot.copy.desktop.tool_aria}>
      <select bind:value={state.tool} onchange={() => void runSearch()}>
        <option value="">{snapshot.copy.tools.all}</option>
        {#each snapshot.tools.filter((tool) => tool.value !== 'all') as tool (tool.value)}
          <option value={tool.value}>{tool.label}</option>
        {/each}
      </select>
    </label>
    <label class="filter-field" aria-label={snapshot.copy.desktop.project_aria ?? snapshot.copy.tables.project}>
      <select bind:value={state.project} onchange={() => void runSearch()}>
        <option value="">{snapshot.copy.tools.all}</option>
        {#each snapshot.projects.filter((project) => project.identity !== null) as project (project.identity)}
          <option value={project.identity}>{project.label}</option>
        {/each}
      </select>
    </label>
    {#if state.results}
      <span class="scrollback-count mono">
        {fill(snapshot.copy.scrollback.result_count, {
          shown: String(state.results.sessions.length),
          total: String(state.results.total_sessions)
        })}
      </span>
    {/if}
  </div>

  {#if snapshot.source === 'sample'}
    <p class="scrollback-note">{snapshot.copy.scrollback.sample_mode_note}</p>
  {/if}
  {#if state.error}
    <p class="scrollback-error" role="alert">{state.error}</p>
  {/if}

  <Panel title={snapshot.copy.scrollback.title} tone="blue">
    {#if !state.results}
      <p class="scrollback-empty">{snapshot.copy.scrollback.empty_state}</p>
    {:else if state.results.sessions.length === 0}
      <p class="scrollback-empty">{snapshot.copy.scrollback.no_results}</p>
    {:else}
      <div class="scrollback-results">
        {#each state.results.sessions as session (session.key)}
          <div
            class="scrollback-group click-row"
            style:--rank-fill={`${matchRank(session)}%`}
            role="button"
            tabindex="0"
            onclick={() => openSession(session.key)}
            onkeydown={(event) => handleRowKey(event, session)}
          >
            <div class="scrollback-session rank-line">
              <span class="scrollback-project">{session.project}</span>
              <span class="scrollback-meta">{toolLabel(session.tool)}</span>
              <span class="scrollback-meta mono">{dateOf(session)}</span>
              <span class="scrollback-meta money mono">{session.cost}</span>
              <span class="scrollback-matches mono">{count(session.match_count)}</span>
              {#if session.prompt_only}
                <span class="scrollback-badge">{snapshot.copy.scrollback.prompt_only_badge}</span>
              {/if}
            </div>
            {#each session.matches as matched (matched.dedup_key + matched.role)}
              <div class="scrollback-snippet">
                <span class="scrollback-role">
                  {matched.role === 'user'
                    ? snapshot.copy.scrollback.role_user
                    : snapshot.copy.scrollback.role_assistant}
                </span>
                <span class="scrollback-text">
                  {#each matched.snippet as part, idx (idx)}
                    {#if part.hit}<mark>{part.text}</mark>{:else}{part.text}{/if}
                  {/each}
                </span>
              </div>
            {/each}
            {#if extraMatches(session) > 0}
              <div class="scrollback-more">
                {fill(snapshot.copy.scrollback.more_matches, {
                  count: String(extraMatches(session))
                })}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </Panel>
</section>

<style>
  .scrollback-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-md);
    align-items: center;
  }

  .scrollback-query {
    flex: 1 1 260px;
  }

  .scrollback-query::after {
    display: none;
  }

  .scrollback-query input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--color-on-surface);
    font: inherit;
  }

  .scrollback-count {
    margin-left: auto;
    color: var(--color-muted);
    font-size: 12px;
  }

  .scrollback-note,
  .scrollback-empty {
    color: var(--color-muted);
    font-size: 13px;
    margin: 0;
  }

  .scrollback-error {
    color: var(--color-error);
    font-size: 13px;
    margin: 0;
  }

  .scrollback-results {
    display: flex;
    flex-direction: column;
    max-height: 480px;
    overflow-y: auto;
  }

  .scrollback-group {
    padding: 6px 8px;
    border-bottom: 1px solid var(--color-border-soft);
    cursor: pointer;
  }

  .scrollback-group:last-child {
    border-bottom: none;
  }

  .scrollback-group:focus-visible {
    outline: 1px solid var(--color-primary);
    outline-offset: -1px;
  }

  .scrollback-session {
    display: flex;
    align-items: baseline;
    gap: var(--space-md);
    background: linear-gradient(
      to right,
      var(--color-rank-fill) 0,
      var(--color-rank-fill) var(--rank-fill),
      transparent var(--rank-fill)
    );
    padding: 2px 4px;
  }

  .scrollback-project {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scrollback-meta {
    color: var(--color-muted);
    font-size: 12px;
    white-space: nowrap;
  }

  .scrollback-matches {
    color: var(--color-secondary);
    font-size: 12px;
  }

  .scrollback-badge {
    color: var(--color-muted-2);
    font-size: 11px;
    border: 1px solid var(--color-border-soft);
    border-radius: 3px;
    padding: 0 4px;
    white-space: nowrap;
  }

  .scrollback-snippet {
    display: flex;
    gap: var(--space-md);
    padding: 1px 4px 1px 16px;
    font-size: 12.5px;
    color: var(--color-muted);
  }

  .scrollback-role {
    flex: 0 0 62px;
    text-align: right;
    color: var(--color-muted-2);
    font-size: 11px;
    line-height: 1.7;
  }

  .scrollback-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scrollback-text mark {
    background: transparent;
    color: var(--color-primary);
    font-weight: 600;
  }

  .scrollback-more {
    padding: 0 4px 2px 16px;
    color: var(--color-muted-2);
    font-size: 11.5px;
  }
</style>
