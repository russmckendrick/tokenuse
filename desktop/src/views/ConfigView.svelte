<script lang="ts">
  import { Database, FolderOpen, Trash2 } from 'lucide-svelte';
  import { reveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { ConfigRow, DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;
  export let configAction: (row: ConfigRow) => void;
  export let chooseExportDir: () => void;
  export let refreshArchive: () => void;
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
              <td class="muted-cell">{row.value}</td>
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
    <Panel title={snapshot.copy.panels.local_data} tone="green">
      <div class="config-facts">
        <div><span>{snapshot.copy.tables.archive}</span><strong>{snapshot.source}</strong></div>
        <div><span>{snapshot.copy.tables.currency}</span><strong>{snapshot.currency}</strong></div>
        <div><span>{snapshot.copy.tables.exports}</span><strong>{snapshot.export_dir}</strong></div>
      </div>
      <div class="button-row">
        <button type="button" onclick={refreshArchive}><Database size={15} /> {snapshot.copy.actions.refresh}</button>
        <button type="button" onclick={chooseExportDir}><FolderOpen size={15} /> {snapshot.copy.actions.folder}</button>
      </div>
    </Panel>
  </section>
</section>
