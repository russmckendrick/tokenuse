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
</script>

<section class="page config-page" use:reveal={{ y: 5 }}>
  <section class="config-grid">
    <Panel title={snapshot.copy.nav.configuration} tone="cyan">
      <table>
        <thead>
          <tr><th>{snapshot.copy.tables.setting}</th><th>{snapshot.copy.tables.value}</th><th></th></tr>
        </thead>
        <tbody>
          {#each snapshot.config_rows as row}
            <tr>
              <td>{row.name}</td>
              <td class="muted-cell">
                <div>{row.value}</div>
                {#if row.links.length}
                  <div class="row-links">
                    {#each row.links as link}
                      <a href={link.url} target="_blank" rel="noreferrer">{link.label}</a>
                    {/each}
                  </div>
                {/if}
              </td>
              <td class="tight">
                <button class="row-action" class:danger={row.action === 'clear'} type="button" onclick={() => configAction(row)}>
                  {#if row.action === 'clear'}<Trash2 size={14} />{/if}
                  {row.action}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
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
    <Panel title={snapshot.copy.panels.local_data} tone="green">
      <div class="config-facts">
        <div><span>{snapshot.copy.tables.archive}</span><strong>{snapshot.source}</strong></div>
        <div><span>{snapshot.copy.tables.currency}</span><strong>{snapshot.currency}</strong></div>
        <div><span>{snapshot.copy.tables.exports}</span><strong>{snapshot.report_dir}</strong></div>
      </div>
      <div class="button-row">
        <button type="button" onclick={refreshArchive}><Database size={15} /> {snapshot.copy.actions.refresh}</button>
        <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
      </div>
    </Panel>
  </section>
</section>
