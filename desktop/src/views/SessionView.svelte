<script lang="ts">
  import { ArrowLeft } from 'lucide-svelte';
  import { count } from '../format';
  import { reveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { DesktopSnapshot, SessionDetail, SessionDetailView } from '../types';

  export let snapshot: DesktopSnapshot;
  export let session: SessionDetailView | null;
  export let closeSession: () => void;
  export let openCallDetail: (call: SessionDetail) => void;
  export let handleCallRowKey: (event: KeyboardEvent, call: SessionDetail) => void;
</script>

<section class="session-page" use:reveal={{ y: 5 }}>
  <div class="session-head">
    <button type="button" onclick={closeSession}><ArrowLeft size={15} /> {snapshot.copy.nav.deep_dive}</button>
    {#if session}
      <div>
        <strong>{session.project}</strong>
        <span>{session.tool} · {session.date_range}</span>
      </div>
    {/if}
  </div>
  {#if session}
    <section class="kpis session-kpis">
      <div><span>{snapshot.copy.metrics.cost}</span><strong>{session.total_cost}</strong><small>{session.total_calls} {snapshot.copy.metrics.calls}</small></div>
      <div><span>{snapshot.copy.metrics.input}</span><strong>{session.total_input}</strong><small>{snapshot.copy.metrics.tokens}</small></div>
      <div><span>{snapshot.copy.metrics.output}</span><strong>{session.total_output}</strong><small>{snapshot.copy.metrics.tokens}</small></div>
      <div><span>{snapshot.copy.metrics.cache_read}</span><strong>{session.total_cache_read}</strong><small>{snapshot.copy.metrics.tokens}</small></div>
    </section>
    <div class="session-panel-area">
      {#if session.note}<div class="status-line">{session.note}</div>{/if}
      <Panel title={snapshot.copy.panels.calls} tone="red">
        <table>
          <thead><tr><th>{snapshot.copy.tables.time}</th><th>{snapshot.copy.tables.model}</th><th>{snapshot.copy.tables.cost}</th><th>{snapshot.copy.tables.in}</th><th>{snapshot.copy.tables.out}</th><th>{snapshot.copy.tables.cache}</th><th>{snapshot.copy.tables.tools}</th><th>{snapshot.copy.tables.prompt}</th></tr></thead>
          <tbody>
            {#each session.calls as call}
              <tr
                class="click-row"
                tabindex="0"
                onclick={() => openCallDetail(call)}
                onkeydown={(event) => handleCallRowKey(event, call)}
              >
                <td>{call.timestamp}</td>
                <td>{call.model}</td>
                <td class="money">{call.cost}</td>
                <td>{count(call.input_tokens)}</td>
                <td>{count(call.output_tokens)}</td>
                <td>{count(call.cache_read + call.cache_write)}</td>
                <td>{call.tools}</td>
                <td class="prompt-cell">{call.prompt}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </Panel>
    </div>
  {:else}
    <div class="session-panel-area">
      <div class="empty-state">{snapshot.copy.session.no_session_selected}</div>
    </div>
  {/if}
</section>
