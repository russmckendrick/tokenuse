<script lang="ts">
  import { Database, Download, FolderOpen, RefreshCw, Trash2 } from 'lucide-svelte';
  import { reveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { ConfigRow, DesktopSnapshot, DesktopUpdateMetadata } from '../types';

  type DesktopUpdateUiState = {
    checking: boolean;
    installing: boolean;
    checked: boolean;
    available: DesktopUpdateMetadata | null;
    message: string | null;
    downloaded: number;
    total: number | null;
  };

  export let snapshot: DesktopSnapshot;
  export let configAction: (row: ConfigRow) => void;
  export let chooseExportDir: () => void;
  export let refreshArchive: () => void;
  export let desktopUpdate: DesktopUpdateUiState;
  export let checkDesktopUpdate: () => void;
  export let installDesktopUpdate: () => void;
  export let setOpenAtLoginFromEvent: (event: Event) => void;
  export let setShowDockOrTaskbarIconFromEvent: (event: Event) => void;

  const GROUPS: Record<string, string[]> = {
    money: ['currency_override', 'rates_json', 'litellm_prices'],
    tools: [
      'claude_statusline',
      'claude_limits',
      'claude_subscription_limits',
      'codex_subscription_limits',
      'copilot_limits'
    ],
    advice: ['advice_tool', 'advice_prompts'],
    data: ['clear_data']
  };

  // Local-vs-subscription pairs that should collapse to whichever side has
  // data. Hides the unconfigured counterpart when the other side is in use,
  // so the section reads as the user's actual integration, not every option.
  const COUNTERPARTS: Record<string, string> = {
    claude_limits: 'claude_subscription_limits',
    claude_subscription_limits: 'claude_limits'
  };

  function rowsFor(group: keyof typeof GROUPS): ConfigRow[] {
    const ids = GROUPS[group];
    return snapshot.config_rows.filter((row) => ids.includes(row.id));
  }

  function configuredPrefixesFor(id: string): string[] {
    const v = snapshot.copy.config.values;
    switch (id) {
      case 'claude_limits':
        return [v.sidecar_found];
      case 'claude_subscription_limits':
      case 'codex_subscription_limits':
      case 'copilot_limits':
        return [v.quota_snapshot_found];
      case 'claude_statusline':
        return ['Installed'];
      default:
        return [];
    }
  }

  function isRowConfigured(row: ConfigRow | undefined): boolean {
    if (!row) return false;
    return configuredPrefixesFor(row.id).some((prefix) => row.value.startsWith(prefix));
  }

  function visibleToolRows(): ConfigRow[] {
    const rows = rowsFor('tools');
    return rows.filter((row) => {
      const counterpartId = COUNTERPARTS[row.id];
      if (!counterpartId) return true;
      if (isRowConfigured(row)) return true;
      const counterpart = rows.find((r) => r.id === counterpartId);
      return !isRowConfigured(counterpart);
    });
  }
</script>

<section class="page config-page" use:reveal={{ y: 5 }}>
  <section class="config-grid">
    {#if rowsFor('money').length}
      <Panel title={snapshot.copy.panels.money_and_data} tone="cyan">
        <div class="config-rows">
          {#each rowsFor('money') as row}
            <div class="config-row">
              <div class="config-row-title">
                <strong>{row.name}</strong>
              </div>
              <div class="config-row-value">
                <div>{row.value}</div>
                {#if row.links.length}
                  <div class="row-links">
                    {#each row.links as link}
                      <a href={link.url} target="_blank" rel="noreferrer">{link.label}</a>
                    {/each}
                  </div>
                {/if}
              </div>
              <div class="config-row-action">
                <button class="row-action" type="button" onclick={() => configAction(row)}>
                  {row.action}
                </button>
              </div>
            </div>
          {/each}
        </div>
      </Panel>
    {/if}

    {#if visibleToolRows().length}
      <Panel title={snapshot.copy.panels.tool_integrations} tone="orange">
        <div class="config-rows">
          {#each visibleToolRows() as row}
            <div class="config-row">
              <div class="config-row-title">
                <strong>{row.name}</strong>
              </div>
              <div class="config-row-value">
                <div>{row.value}</div>
                {#if row.links.length}
                  <div class="row-links">
                    {#each row.links as link}
                      <a href={link.url} target="_blank" rel="noreferrer">{link.label}</a>
                    {/each}
                  </div>
                {/if}
              </div>
              <div class="config-row-action">
                <button class="row-action" type="button" onclick={() => configAction(row)}>
                  {row.action}
                </button>
              </div>
            </div>
          {/each}
        </div>
      </Panel>
    {/if}

    {#if rowsFor('advice').length}
      <Panel title={snapshot.copy.panels.advice_engine} tone="magenta">
        <div class="config-rows">
          {#each rowsFor('advice') as row}
            <div class="config-row">
              <div class="config-row-title">
                <strong>{row.name}</strong>
              </div>
              <div class="config-row-value">
                <div>{row.value}</div>
              </div>
              <div class="config-row-action">
                <button class="row-action" type="button" onclick={() => configAction(row)}>
                  {row.action}
                </button>
              </div>
            </div>
          {/each}
        </div>
      </Panel>
    {/if}

    <Panel title={snapshot.copy.panels.local_data} tone="green">
      <div class="config-facts">
        <div><span>{snapshot.copy.tables.archive}</span><strong>{snapshot.source}</strong></div>
        <div><span>{snapshot.copy.tables.currency}</span><strong>{snapshot.currency}</strong></div>
        <div><span>{snapshot.copy.tables.exports}</span><strong>{snapshot.report_dir}</strong></div>
      </div>
      <div class="button-row">
        <button type="button" onclick={refreshArchive}><Database size={15} /> {snapshot.copy.actions.refresh}</button>
        <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
        {#each rowsFor('data') as row}
          <button class="row-action danger" type="button" onclick={() => configAction(row)}>
            <Trash2 size={14} /> {row.action}
          </button>
        {/each}
      </div>
    </Panel>

    <Panel title={snapshot.copy.panels.desktop} tone="orange">
      <div class="toggle-list">
        <label class="toggle-row">
          <span>
            <strong>{snapshot.copy.desktop.open_at_login}</strong>
            <small>{snapshot.desktop_settings.open_at_login ? snapshot.copy.desktop.enabled : snapshot.copy.desktop.disabled}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={snapshot.desktop_settings.open_at_login}
            onchange={setOpenAtLoginFromEvent}
          />
          <i aria-hidden="true"></i>
        </label>
        <label class="toggle-row">
          <span>
            <strong>{snapshot.copy.desktop.dock_taskbar_icon}</strong>
            <small>{snapshot.desktop_settings.show_dock_or_taskbar_icon ? snapshot.copy.desktop.shown : snapshot.copy.desktop.hidden}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={snapshot.desktop_settings.show_dock_or_taskbar_icon}
            onchange={setShowDockOrTaskbarIconFromEvent}
          />
          <i aria-hidden="true"></i>
        </label>
      </div>
    </Panel>

    {#if snapshot.desktop_updates.supported}
      <Panel title={snapshot.copy.updates.title} tone="magenta">
        <div class="update-panel">
          <div class="update-summary">
            <strong>{snapshot.copy.updates.description}</strong>
            <small>{snapshot.copy.updates.current_version.replace('{version}', snapshot.version)}</small>
          </div>
          {#if desktopUpdate.message}
            <div class="update-status">{desktopUpdate.message}</div>
          {/if}
          <div class="button-row">
            <button type="button" disabled={desktopUpdate.checking || desktopUpdate.installing} onclick={checkDesktopUpdate}>
              <RefreshCw size={15} /> {desktopUpdate.checking ? snapshot.copy.updates.checking : snapshot.copy.updates.check}
            </button>
            {#if desktopUpdate.available}
              <button type="button" disabled={desktopUpdate.installing} onclick={installDesktopUpdate}>
                <Download size={15} /> {desktopUpdate.installing ? snapshot.copy.updates.installing : snapshot.copy.updates.install}
              </button>
            {/if}
          </div>
        </div>
      </Panel>
    {/if}
  </section>
</section>
