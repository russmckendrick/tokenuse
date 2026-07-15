<script lang="ts">
  import { api } from '../api';
  import GanttRows from '../charts/GanttRows.svelte';
  import RankBar from '../components/RankBar.svelte';
  import ScoreBar from '../components/ScoreBar.svelte';
  import { count } from '../format';
  import { staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type { CoachData, CoachTimelineDay, DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;

  let coach: CoachData | null = null;
  let coachKey = '';
  let timeline: CoachTimelineDay | null = null;
  let selectedDay = '';
  let loadedDay = '';

  $: coachCopy = snapshot.copy.coach;
  $: {
    const key = [
      snapshot.period,
      snapshot.tool,
      snapshot.project.identity ?? '',
      snapshot.data_generation,
      snapshot.currency
    ].join('|');
    if (key !== coachKey) {
      coachKey = key;
      void loadCoach();
    }
  }
  $: if (coach && selectedDay && selectedDay !== loadedDay) {
    loadedDay = selectedDay;
    void loadTimeline(selectedDay);
  }

  async function loadCoach() {
    try {
      const data = await api.getCoach(snapshot.period);
      coach = data;
      if (!data.timeline_days.includes(selectedDay)) {
        selectedDay = data.timeline_days[0] ?? '';
        loadedDay = '';
        timeline = null;
      } else {
        loadedDay = '';
      }
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  async function loadTimeline(day: string) {
    try {
      timeline = await api.getCoachTimeline(day);
    } catch {
      timeline = null;
    }
  }

  function tpl(text: string, vars: Record<string, string | number>): string {
    return text.replace(/\{(\w+)\}/g, (raw, key: string) =>
      key in vars ? String(vars[key]) : raw
    );
  }

  function ruleCopy(ruleId: string) {
    return (
      coachCopy.rules[ruleId] ?? {
        name: ruleId,
        description: '',
        when_triggered: '',
        how_to_improve: ''
      }
    );
  }

  function findingLine(finding: {
    rule_id: string;
    occurrences: number;
    total: number;
    pct: string;
    stat: string;
  }): string {
    return tpl(ruleCopy(finding.rule_id).when_triggered, {
      count: count(finding.occurrences),
      total: count(finding.total),
      pct: finding.pct,
      stat: finding.stat
    });
  }
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  {#if coach}
    <section class="score-grid">
      {#each coach.practice_groups as group (group.id)}
        <Panel title={coachCopy.groups[group.id] ?? group.id} tone="blue">
          <div class="score-card">
            <div class="score-value mono">{group.score}</div>
            <ScoreBar score={group.score} ariaLabel={coachCopy.groups[group.id] ?? group.id} />
            <div class="score-meta mono">
              <span>{coachCopy.score.wow} {group.wow}</span>
              <span>{coachCopy.score.mom} {group.mom}</span>
            </div>
            <div class="score-detail">
              {tpl(coachCopy.score.rules_triggered, {
                count: group.triggered,
                total: group.total_rules
              })}
            </div>
            <div class="score-detail dim">
              {#if group.top_rule_id}
                {coachCopy.score.top_issue}: {ruleCopy(group.top_rule_id).name}
              {:else}
                {coachCopy.score.clean}
              {/if}
            </div>
          </div>
        </Panel>
      {/each}
    </section>

    <Panel title={coachCopy.findings.title} tone="red" scrollable>
      {#if coach.findings.length === 0}
        <p class="coach-empty">{coachCopy.findings.empty}</p>
      {:else}
        <ul class="finding-list">
          {#each coach.findings as finding (finding.rule_id)}
            <li class="finding-row">
              <details>
                <summary>
                  <span class={`severity-badge ${finding.severity}`}>
                    {coachCopy.findings.severity[finding.severity] ?? finding.severity}
                  </span>
                  <span class="finding-name">{ruleCopy(finding.rule_id).name}</span>
                  <span class="finding-group dim">{coachCopy.groups[finding.group] ?? finding.group}</span>
                  <span class="finding-count mono">
                    {tpl(coachCopy.findings.occurrences, { count: count(finding.occurrences) })}
                  </span>
                </summary>
                <div class="finding-body">
                  <p>{findingLine(finding)}</p>
                  <p class="finding-improve">
                    <strong>{coachCopy.findings.improve}:</strong>
                    {ruleCopy(finding.rule_id).how_to_improve}
                  </p>
                  {#if finding.examples.length > 0}
                    <p class="dim">{coachCopy.findings.examples}:</p>
                    <ul class="finding-examples">
                      {#each finding.examples as example}
                        <li>
                          <span class="mono">"{example.text}"</span>
                          {#if example.detail}
                            <span class="dim"> — {example.detail}</span>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </div>
              </details>
            </li>
          {/each}
        </ul>
      {/if}
    </Panel>

    <section class="duo-grid">
      <Panel title={coachCopy.flow.title} tone="cyan">
        {#if coach.flow.total_days === 0}
          <p class="coach-empty">{coachCopy.flow.empty}</p>
        {:else}
          <div class="stat-row">
            <div class="stat">
              <span class="stat-value mono">{coach.flow.overall_score}</span>
              <span class="stat-label">{coachCopy.flow.labels[coach.flow.label_id] ?? coach.flow.label_id}</span>
            </div>
            <div class="stat">
              <span class="stat-value mono">{coach.flow.avg_followup}</span>
              <span class="stat-label">{coachCopy.flow.avg_followup}</span>
            </div>
            <div class="stat">
              <span class="stat-value mono">{coach.flow.avg_block}</span>
              <span class="stat-label">{coachCopy.flow.avg_block}</span>
            </div>
            <div class="stat">
              <span class="stat-value mono">{coach.flow.deep_days}/{coach.flow.total_days}</span>
              <span class="stat-label">{coachCopy.flow.deep_days}</span>
            </div>
          </div>
          <ul class="flow-days">
            {#each coach.flow.days as day (day.day)}
              <li>
                <span class="mono flow-day">{day.day}</span>
                <ScoreBar score={day.score} ariaLabel={day.day} />
                <span class="mono flow-score">{day.score}</span>
                <span class="dim flow-label">{coachCopy.flow.labels[day.label_id] ?? day.label_id}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </Panel>

      <Panel title={coachCopy.pace.title} tone="green">
        <div class="stat-row">
          <div class="stat">
            <span class="stat-value mono">{coach.pace.current_streak}</span>
            <span class="stat-label">{coachCopy.pace.streak}</span>
          </div>
          <div class="stat">
            <span class="stat-value mono">{coach.pace.longest_streak}</span>
            <span class="stat-label">{coachCopy.pace.longest_streak}</span>
          </div>
          <div class="stat">
            <span class="stat-value mono">{coach.pace.late_night_pct}%</span>
            <span class="stat-label">{coachCopy.pace.late_night}</span>
          </div>
          <div class="stat">
            <span class="stat-value mono">{coach.pace.weekend_pct}%</span>
            <span class="stat-label">{coachCopy.pace.weekend}</span>
          </div>
        </div>
        <div class="pace-risk">
          <span class="stat-label">{coachCopy.pace.risk}:</span>
          <span class={`severity-badge ${coach.pace.risk_id === 'high' ? 'high' : coach.pace.risk_id === 'medium' ? 'medium' : 'low'}`}>
            {coachCopy.pace.risks[coach.pace.risk_id] ?? coach.pace.risk_id}
          </span>
        </div>
        {#if coach.pace.alert_ids.length > 0}
          <ul class="pace-alerts">
            {#each coach.pace.alert_ids as alert (alert)}
              <li>{coachCopy.pace.alerts[alert] ?? alert}</li>
            {/each}
          </ul>
        {/if}
      </Panel>
    </section>

    <Panel title={coachCopy.timeline.title} tone="orange">
      {#if coach.timeline_days.length === 0}
        <p class="coach-empty">{coachCopy.timeline.empty}</p>
      {:else}
        <div class="timeline-controls">
          <label class="stat-label" for="coach-timeline-day">{coachCopy.timeline.day}</label>
          <select id="coach-timeline-day" bind:value={selectedDay}>
            {#each coach.timeline_days as day (day)}
              <option value={day}>{day}</option>
            {/each}
          </select>
          {#if timeline && timeline.max_concurrent > 1}
            <span class="overlap-badge mono">
              {tpl(coachCopy.timeline.overlap, { count: timeline.max_concurrent })}
            </span>
          {/if}
        </div>
        {#if timeline}
          <GanttRows
            rows={timeline.rows}
            windowStartMin={timeline.window_start_min}
            windowEndMin={timeline.window_end_min}
            ariaLabel={coachCopy.timeline.title}
            emptyLabel={coachCopy.timeline.empty}
            turnsLabel={(n) => tpl(coachCopy.timeline.turns, { count: n })}
          />
        {:else}
          <p class="coach-empty">{coachCopy.timeline.empty}</p>
        {/if}
      {/if}
    </Panel>

    <Panel title={coachCopy.output.title} tone="magenta" scrollable>
      {#if coach.output.by_language.length === 0}
        <p class="coach-empty">{coachCopy.output.empty}</p>
      {:else}
        <div class="output-total">
          <span class="stat-value mono">{coach.output.total_loc}</span>
          <span class="stat-label">{coachCopy.output.total}</span>
          {#if coach.output.uncovered_tools}
            <span class="dim coverage-note">
              {tpl(coachCopy.output.coverage_note, { tools: coach.output.uncovered_tools })}
            </span>
          {/if}
        </div>
        <section class="duo-grid">
          <div>
            <h3 class="output-heading">{coachCopy.output.by_language}</h3>
            <ul class="output-list">
              {#each coach.output.by_language as row (row.name)}
                <li>
                  <RankBar value={row.value} ariaLabel={row.name} compact />
                  <span class="output-name">{row.name}</span>
                  <span class="mono output-count">{count(row.calls)}</span>
                </li>
              {/each}
            </ul>
            <h3 class="output-heading">{coachCopy.output.by_day}</h3>
            <ul class="output-list">
              {#each coach.output.by_day as row (row.name)}
                <li>
                  <RankBar value={row.value} ariaLabel={row.name} compact />
                  <span class="output-name mono">{row.name}</span>
                  <span class="mono output-count">{count(row.calls)}</span>
                </li>
              {/each}
            </ul>
          </div>
          <div>
            <h3 class="output-heading">{coachCopy.output.by_project}</h3>
            <ul class="output-list">
              {#each coach.output.by_project as row (row.name)}
                <li>
                  <RankBar value={row.value} ariaLabel={row.name} compact />
                  <span class="output-name">{row.name}</span>
                  <span class="mono output-count">{count(row.calls)}</span>
                </li>
              {/each}
            </ul>
            <h3 class="output-heading">{coachCopy.output.by_model}</h3>
            <ul class="output-list">
              {#each coach.output.by_model as row (row.name)}
                <li>
                  <RankBar value={row.value} ariaLabel={row.name} compact />
                  <span class="output-name">{row.name}</span>
                  <span class="mono output-count">{count(row.calls)}</span>
                </li>
              {/each}
            </ul>
          </div>
        </section>
      {/if}
    </Panel>
  {/if}
</section>

<style>
  .score-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-panel-gap, 12px);
  }

  @media (max-width: 1100px) {
    .score-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  .score-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .score-value {
    font-size: var(--text-display-lg);
    font-weight: 600;
  }

  .score-meta {
    display: flex;
    gap: 12px;
    font-size: var(--text-label);
    color: var(--color-muted);
  }

  .score-detail {
    font-size: var(--text-label);
    color: var(--color-on-surface);
  }

  .dim {
    color: var(--color-muted-2);
  }

  .coach-empty {
    color: var(--color-muted);
    font-size: var(--text-body);
    margin: 0;
  }

  .finding-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .finding-row {
    border-bottom: 1px solid var(--color-border-row);
  }

  .finding-row:last-child {
    border-bottom: none;
  }

  .finding-row summary {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
    cursor: pointer;
    list-style: none;
  }

  .finding-name {
    color: var(--color-on-surface);
    font-size: var(--text-body);
  }

  .finding-group {
    font-size: var(--text-label);
  }

  .finding-count {
    margin-left: auto;
    font-size: var(--text-label);
    color: var(--color-muted);
  }

  .finding-body {
    padding: 0 0 10px 0;
    font-size: var(--text-body);
    color: var(--color-on-surface);
  }

  .finding-body p {
    margin: 4px 0;
  }

  .finding-improve {
    color: var(--color-muted);
  }

  .finding-examples {
    margin: 4px 0 0 0;
    padding-left: 16px;
    font-size: var(--text-label);
  }

  .severity-badge {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 1px 6px;
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    white-space: nowrap;
  }

  .severity-badge.high {
    color: var(--color-error);
    border-color: var(--color-error);
  }

  .severity-badge.medium {
    color: var(--color-warning);
    border-color: var(--color-warning);
  }

  .severity-badge.low {
    color: var(--color-tertiary);
    border-color: var(--color-tertiary);
  }

  .stat-row {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin-bottom: 10px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stat-value {
    font-size: var(--text-display);
    font-weight: 600;
  }

  .stat-label {
    font-size: var(--text-label);
    color: var(--color-muted-2);
  }

  .flow-days {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .flow-days li {
    display: grid;
    grid-template-columns: 84px 1fr 32px 80px;
    align-items: center;
    gap: 8px;
    font-size: var(--text-label);
  }

  .flow-score {
    text-align: right;
  }

  .pace-risk {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .pace-alerts {
    margin: 4px 0 0 0;
    padding-left: 16px;
    font-size: var(--text-label);
    color: var(--color-warning);
  }

  .timeline-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }

  .overlap-badge {
    font-size: var(--text-label);
    color: var(--color-warning);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-pill);
    padding: 1px 8px;
  }

  .output-total {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 10px;
  }

  .coverage-note {
    font-size: var(--text-label);
  }

  .output-heading {
    font-size: var(--text-label);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-muted-2);
    margin: 10px 0 6px 0;
  }

  .output-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .output-list li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-label);
  }

  .output-name {
    color: var(--color-on-surface);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-count {
    margin-left: auto;
    color: var(--color-muted);
  }
</style>
