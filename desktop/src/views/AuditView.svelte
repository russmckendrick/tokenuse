<script lang="ts">
  import { RefreshCw, ShieldAlert } from 'lucide-svelte';
  import { reveal, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type {
    AuditFinding,
    AuditKnowledgeFile,
    AuditRankedItem,
    AuditSection,
    AuditSeverity,
    DesktopSnapshot
  } from '../types';

  export let snapshot: DesktopSnapshot;
  export let refreshAudit: () => void;

  $: audit = snapshot.audit;
  $: copy = snapshot.copy.audit;

  function sectionLabel(section: AuditSection) {
    return copy.sections[section] ?? section;
  }

  function severityLabel(severity: AuditSeverity) {
    return copy.severity[severity] ?? severity;
  }

  function severityClass(severity: AuditSeverity) {
    return `severity-${severity}`;
  }

  function measured(value: number | null, suffix = '') {
    return value === null ? '-' : `${value}${suffix}`;
  }

  function knowledgeTags(file: AuditKnowledgeFile) {
    const tags: string[] = [];
    if (file.feature_flags.mentions_testing) tags.push('tests');
    if (file.feature_flags.mentions_security) tags.push('security');
    if (file.feature_flags.imports_other_files) tags.push('imports');
    if (file.content_truncated) tags.push('large');
    return tags.length ? tags.join(' · ') : '-';
  }

  function findingEvidence(finding: AuditFinding) {
    return finding.evidence.length ? finding.evidence.join(' · ') : '-';
  }

  function usageValue(value: number, available: boolean) {
    return available ? String(value) : copy.not_measured;
  }

  function usageCost(label: string, available: boolean) {
    return available ? label : copy.no_archive_data;
  }

  function recentCost() {
    if (!audit.recent_usage.available) return copy.no_archive_data;
    return audit.recent_usage.calls === 0 ? copy.no_recent_calls : audit.recent_usage.cost_label;
  }

  function rankedNames(items: AuditRankedItem[]) {
    return items.length ? items.map((item) => `${item.name} ${item.cost_label}`).join(' · ') : '-';
  }

  function coverageRatio() {
    if (!audit.project_coverage.available) return copy.no_archive_data;
    if (audit.project_coverage.checked_project_roots === 0) return copy.no_readable_project_roots;
    const rootsWithInstructions = audit.project_coverage.roots_with_agent_instructions ?? 0;
    return `${rootsWithInstructions}/${audit.project_coverage.checked_project_roots}`;
  }
</script>

<section class="page audit-page" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  <header class="audit-header" use:reveal={{ y: 4 }}>
    <div>
      <h1>{copy.title}</h1>
      <p>{copy.subtitle}</p>
    </div>
    <button type="button" onclick={refreshAudit}>
      <RefreshCw size={15} />
      {copy.refresh}
    </button>
  </header>

  <section class="audit-kpis" aria-label={copy.title}>
    <div><span>{copy.captured_at}</span><strong>{audit.captured_at || copy.not_captured}</strong></div>
    <div><span>{copy.primary_tool}</span><strong>{audit.primary_tool_guess ?? '-'}</strong></div>
    <div><span>{snapshot.copy.tables.findings}</span><strong>{audit.summary.total_findings}</strong></div>
    <div><span>{copy.redaction}</span><strong>{audit.redaction.enabled ? 'on' : 'off'}</strong></div>
  </section>

  <section class="audit-grid">
    <Panel title={copy.findings_title} tone="orange">
      {#if audit.findings.length === 0}
        <div class="audit-empty">{copy.no_findings}</div>
      {:else}
        <div class="audit-findings">
          {#each audit.findings as finding}
            <article class="audit-finding">
              <div class="finding-head">
                <span class={`severity ${severityClass(finding.severity)}`}>
                  {severityLabel(finding.severity)}
                </span>
                <span>{sectionLabel(finding.section)}</span>
              </div>
              <strong>{finding.title}</strong>
              <p>{finding.body}</p>
              <small>{findingEvidence(finding)}</small>
            </article>
          {/each}
        </div>
      {/if}
    </Panel>

    <div class="audit-side">
      <Panel title={copy.tools_title} tone="cyan">
        <div class="audit-table">
          {#each audit.tools as tool}
            <div class="audit-row">
              <strong>{tool.label}</strong>
              <span>{tool.present ? 'present' : 'missing'}</span>
              <small>{tool.mcp_servers.length} MCP · {tool.config_paths.length} config · {tool.scoped_assets} scoped</small>
            </div>
          {/each}
        </div>
      </Panel>

      <Panel title={copy.behavior_title} tone="magenta">
        <div class="audit-facts">
          <div><span>logs</span><strong>{audit.behavior.recent_sessions_inspected}</strong></div>
          <div><span>clear</span><strong>{measured(audit.behavior.clear_uses)}</strong></div>
          <div><span>compact</span><strong>{measured(audit.behavior.compact_uses)}</strong></div>
          <div><span>subagents</span><strong>{measured(audit.behavior.subagent_calls)}</strong></div>
          <div><span>plan mode</span><strong>{measured(audit.behavior.plan_mode_uses)}</strong></div>
          <div><span>avg turn</span><strong>{measured(audit.behavior.avg_user_turn_chars, ' chars')}</strong></div>
        </div>
      </Panel>

      <Panel title={copy.project_title} tone="green">
        <div class="audit-facts">
          <div><span>{copy.all_time}</span><strong>{usageCost(audit.usage_summary.cost_label, audit.usage_summary.available)}</strong></div>
          <div><span>{copy.recent_7d}</span><strong>{recentCost()}</strong></div>
          <div><span>calls</span><strong>{usageValue(audit.usage_summary.calls, audit.usage_summary.available)}</strong></div>
          <div><span>sessions</span><strong>{usageValue(audit.usage_summary.sessions, audit.usage_summary.available)}</strong></div>
          <div class="wide"><span>top tools</span><strong>{rankedNames(audit.usage_summary.top_tools)}</strong></div>
          <div class="wide"><span>top projects</span><strong>{rankedNames(audit.recent_usage.top_projects)}</strong></div>
        </div>
      </Panel>

      <Panel title={copy.coverage_title} tone="blue">
        <div class="audit-facts">
          <div><span>known roots</span><strong>{audit.project_coverage.available ? audit.project_coverage.known_project_roots : copy.no_archive_data}</strong></div>
          <div><span>checked</span><strong>{audit.project_coverage.available ? audit.project_coverage.checked_project_roots : copy.no_archive_data}</strong></div>
          <div><span>with instructions</span><strong>{coverageRatio()}</strong></div>
          <div><span>with CI</span><strong>{audit.project_coverage.available ? audit.project_coverage.roots_with_ci : copy.no_archive_data}</strong></div>
          <div><span>activity tools</span><strong>{audit.activity_signals.available ? audit.activity_signals.tool_call_uses : copy.no_archive_data}</strong></div>
          <div><span>MCP calls</span><strong>{audit.activity_signals.available ? audit.activity_signals.mcp_tool_uses : copy.no_archive_data}</strong></div>
        </div>
      </Panel>
    </div>
  </section>

  <Panel title={copy.knowledge_title} tone="blue">
    <div class="knowledge-list">
      {#each audit.knowledge_files as file}
        <div class="knowledge-row">
          <ShieldAlert size={14} />
          <strong>{file.path}</strong>
          <span>{file.line_count} lines</span>
          <small>{knowledgeTags(file)}</small>
        </div>
      {/each}
    </div>
  </Panel>
</section>

<style>
  .audit-page {
    height: 100%;
    min-height: 0;
    display: grid;
    grid-auto-rows: min-content;
    align-content: start;
    gap: var(--space-lg);
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    padding-right: 4px;
    padding-bottom: var(--space-lg);
  }

  .audit-page :global(.panel) {
    grid-template-rows: auto auto;
  }

  .audit-page :global(.panel-body) {
    overflow: visible;
    scrollbar-gutter: auto;
  }

  .audit-header,
  .audit-kpis,
  .audit-grid,
  .audit-side {
    display: grid;
    gap: var(--space-lg);
  }

  .audit-header {
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    border: 1px solid var(--color-border);
    background: var(--color-neutral);
    border-radius: var(--radius-md);
    padding: var(--space-lg);
  }

  .audit-header h1 {
    margin: 0 0 4px;
    font-size: 14px;
    color: var(--color-primary);
  }

  .audit-header p {
    margin: 0;
    color: var(--color-muted);
    max-width: 76ch;
  }

  .audit-header button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .audit-kpis {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .audit-kpis > div,
  .audit-facts > div {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: 4px;
    border: 1px solid var(--color-border);
    background: var(--color-neutral);
    border-radius: var(--radius-md);
    padding: var(--space-md);
  }

  .audit-kpis span,
  .audit-facts span {
    color: var(--color-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .audit-kpis strong,
  .audit-facts strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-on-surface);
  }

  .audit-grid {
    grid-template-columns: minmax(500px, 1.25fr) minmax(380px, 0.85fr);
    align-items: start;
  }

  .audit-side {
    align-content: start;
    gap: var(--space-md);
  }

  .audit-findings,
  .audit-table,
  .knowledge-list {
    display: grid;
    gap: 8px;
  }

  .audit-finding,
  .audit-row,
  .knowledge-row,
  .audit-empty {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.02);
    padding: var(--space-md);
  }

  .finding-head,
  .knowledge-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .audit-row {
    display: grid;
    grid-template-columns: minmax(110px, 0.8fr) auto minmax(0, 1fr);
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .finding-head {
    color: var(--color-muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin-bottom: 6px;
  }

  .audit-finding strong,
  .audit-row strong,
  .knowledge-row strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .audit-finding p {
    margin: 6px 0;
    color: var(--color-on-surface);
  }

  .audit-finding small,
  .audit-row small,
  .knowledge-row small,
  .audit-row span,
  .knowledge-row span {
    color: var(--color-muted);
  }

  .severity {
    font-size: 11px;
    font-weight: 700;
  }

  .severity-risk {
    color: var(--color-error);
  }

  .severity-warning {
    color: var(--color-warning);
  }

  .severity-info {
    color: var(--color-secondary);
  }

  .audit-facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .audit-facts > .wide {
    grid-column: 1 / -1;
  }

  .audit-facts > .wide strong {
    white-space: normal;
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .knowledge-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto minmax(90px, auto);
  }

  @media (max-width: 980px) {
    .audit-grid,
    .audit-kpis {
      grid-template-columns: 1fr;
    }

    .audit-header {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 1180px) {
    .audit-grid {
      grid-template-columns: 1fr;
    }

    .audit-side {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 760px) {
    .audit-page {
      padding-right: 0;
    }

    .audit-side,
    .audit-kpis,
    .audit-facts {
      grid-template-columns: 1fr;
    }

    .knowledge-row {
      grid-template-columns: auto minmax(0, 1fr);
    }

    .knowledge-row span,
    .knowledge-row small {
      grid-column: 2;
    }
  }
</style>
