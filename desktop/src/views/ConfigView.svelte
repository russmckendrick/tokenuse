<script lang="ts">
  import { Database, Download, FolderOpen, RefreshCw, Stethoscope, Trash2 } from 'lucide-svelte';
  import { api } from '../api';
  import Badge from '../components/Badge.svelte';
  import { reveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type {
    ConfigRow,
    DesktopSnapshot,
    DesktopUpdateMetadata,
    DoctorReport,
    DoctorToolReport,
    DoctorVerdict
  } from '../types';

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
  export let toggleSampleData: () => void;
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

  // Doctor diagnostics re-walk every adapter's probe roots and parse a
  // bounded sample, so they run on demand only; the last result and its
  // timestamp live in component state, never in the snapshot poll.
  let doctor: DoctorReport | null = null;
  let doctorRunning = false;
  let doctorRanAt: string | null = null;
  let doctorError: string | null = null;

  async function runDoctor() {
    doctorRunning = true;
    doctorError = null;
    try {
      doctor = await api.getDoctor();
      doctorRanAt = new Date().toLocaleTimeString();
    } catch (error) {
      doctorError = String(error);
    } finally {
      doctorRunning = false;
    }
  }

  function template(text: string, values: Record<string, string>): string {
    return Object.entries(values).reduce(
      (out, [key, value]) => out.split(`{${key}}`).join(value),
      text
    );
  }

  function verdictLabel(verdict: DoctorVerdict): string {
    const d = snapshot.copy.doctor;
    switch (verdict) {
      case 'ok':
        return d.verdict_ok;
      case 'nothing_found':
        return d.verdict_nothing_found;
      case 'errors':
        return d.verdict_errors;
      case 'discovery_failed':
        return d.verdict_discovery_failed;
    }
  }

  function verdictTone(verdict: DoctorVerdict): 'good' | 'warn' | 'bad' | 'neutral' {
    switch (verdict) {
      case 'ok':
        return 'good';
      case 'nothing_found':
        return 'neutral';
      default:
        return 'bad';
    }
  }

  function sourcesLine(tool: DoctorToolReport): string {
    return template(snapshot.copy.doctor.sources_line, {
      sessions: String(tool.session_sources),
      limits: String(tool.limit_sources)
    });
  }

  function sampleLine(tool: DoctorToolReport): string {
    return template(snapshot.copy.doctor.sample_line, {
      sources: String(tool.sampled_sources),
      calls: String(tool.sampled_calls),
      snapshots: String(tool.sampled_limit_snapshots),
      errors: String(tool.parse_errors)
    });
  }

  function hasSample(tool: DoctorToolReport): boolean {
    return tool.sampled_sources > 0 || tool.sampled_limit_snapshots > 0 || tool.parse_errors > 0;
  }

  // The shared pricing row appends the fallback-pricing warning to its value
  // (all-time scope). Tone the row from its own text — via the copy template
  // prefix — so the treatment can never disagree with what the row says.
  function rowWarns(row: ConfigRow): boolean {
    if (row.id !== 'litellm_prices') return false;
    const prefix = snapshot.copy.config.values.fallback_priced_models.split('{models}')[0].trim();
    return prefix.length > 0 && row.value.includes(prefix);
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
                <div class:value-warn={rowWarns(row)}>{row.value}</div>
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

    <Panel title={snapshot.copy.panels.local_data} tone="green">
      <div class="toggle-list">
        <label class="toggle-row">
          <span>
            <strong class="sample-data-label">{snapshot.copy.status.sample_data}</strong>
            <small>{snapshot.source === 'sample' ? snapshot.copy.desktop.enabled : snapshot.copy.desktop.disabled}</small>
          </span>
          <input
            type="checkbox"
            role="switch"
            checked={snapshot.source === 'sample'}
            onchange={toggleSampleData}
          />
          <i aria-hidden="true"></i>
        </label>
      </div>
      <div class="config-facts">
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

    <Panel title={snapshot.copy.doctor.panel_title} tone="blue">
      <div class="update-panel">
        <div class="doctor-head">
          <div class="update-summary doctor-summary">
            <strong>{snapshot.copy.doctor.subtitle}</strong>
            {#if doctorRanAt}
              <small>{template(snapshot.copy.doctor.last_run, { time: doctorRanAt })}</small>
            {/if}
          </div>
          <button type="button" disabled={doctorRunning} onclick={runDoctor}>
            <Stethoscope size={15} />
            {doctorRunning ? snapshot.copy.doctor.running : snapshot.copy.doctor.run}
          </button>
        </div>
        {#if doctorError}
          <div class="update-status">{doctorError}</div>
        {/if}
      </div>
      {#if doctor}
        <div class="doctor-rows">
          {#each doctor.tools as tool (tool.id)}
            <div class="doctor-row">
              <div class="doctor-row-head">
                <strong>{tool.name}</strong>
                <span class="doctor-sources mono">{sourcesLine(tool)}</span>
                <Badge label={verdictLabel(tool.verdict)} tone={verdictTone(tool.verdict)} />
              </div>
              {#if tool.detail}
                <div class="doctor-detail">{tool.detail}</div>
              {/if}
              {#if hasSample(tool)}
                <div class="doctor-sample mono">{sampleLine(tool)}</div>
              {/if}
              <ul class="doctor-locations">
                {#each tool.roots as root (root.path)}
                  <li>
                    <span class="doctor-state" class:found={root.exists}>
                      {root.exists ? snapshot.copy.doctor.root_found : snapshot.copy.doctor.root_missing}
                    </span>
                    <span class="doctor-path mono">{root.path}</span>
                  </li>
                {/each}
                {#each tool.env as env (env.name)}
                  <li>
                    <span class="doctor-path mono">
                      {env.value !== null
                        ? `${env.name}=${env.value}`
                        : `${env.name} (${snapshot.copy.doctor.env_not_set})`}
                    </span>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      {/if}
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

<style>
  .sample-data-label {
    text-transform: capitalize;
  }

  .doctor-head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-lg);
  }

  .doctor-summary {
    padding: 0;
  }

  .doctor-rows {
    display: grid;
    gap: 0;
    border-top: 1px solid var(--color-border-row);
  }

  .doctor-row {
    display: grid;
    gap: 4px;
    padding: 10px var(--space-lg);
    border-bottom: 1px solid var(--color-border-row);
  }

  .doctor-row:last-child {
    border-bottom: 0;
  }

  .doctor-row-head {
    display: flex;
    align-items: center;
    gap: var(--space-lg);
  }

  .doctor-row-head strong {
    font-family: var(--font-ui);
    font-size: 13px;
    font-weight: 600;
    color: var(--color-on-surface);
  }

  .doctor-sources {
    margin-left: auto;
    font-size: 11px;
    color: var(--color-muted);
    white-space: nowrap;
  }

  .doctor-detail {
    font-family: var(--font-ui);
    font-size: 11px;
    color: var(--color-muted);
  }

  .doctor-sample {
    font-size: 11px;
    color: var(--color-muted);
  }

  .doctor-locations {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }

  .doctor-locations li {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .doctor-state {
    flex: none;
    font-family: var(--font-ui);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-muted);
  }

  .doctor-state.found {
    color: var(--color-tertiary);
  }

  .doctor-path {
    font-size: 11px;
    color: var(--color-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value-warn {
    color: var(--color-warning);
  }
</style>
