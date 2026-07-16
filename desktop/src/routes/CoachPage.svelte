<script lang="ts">
  import { CalendarDays, ChevronRight, Code2, FileText, Lightbulb, Search, TriangleAlert } from 'lucide-svelte';
  import { api } from '../api';
  import ActivityHourGrid from '../charts/ActivityHourGrid.svelte';
  import ActivityProfile from '../charts/ActivityProfile.svelte';
  import CalendarBars from '../charts/CalendarBars.svelte';
  import SessionTimeline from '../charts/SessionTimeline.svelte';
  import OutputTrend from '../charts/OutputTrend.svelte';
  import ActivityPulse from '../components/ActivityPulse.svelte';
  import Badge from '../components/Badge.svelte';
  import GaugeBar from '../components/GaugeBar.svelte';
  import RadialGauge from '../components/RadialGauge.svelte';
  import RankBar from '../components/RankBar.svelte';
  import ScoreBar from '../components/ScoreBar.svelte';
  import Sparkline from '../components/Sparkline.svelte';
  import StatTile from '../components/StatTile.svelte';
  import { count } from '../format';
  import { countUp, staggeredReveal } from '../motion';
  import Panel from '../Panel.svelte';
  import type {
    AnalyticsData,
    CoachData,
    CoachTimelineDay,
    CountMetric,
    DesktopSnapshot,
    SessionDetailView
  } from '../types';

  export let snapshot: DesktopSnapshot;

  let coach: CoachData | null = null;
  let analytics: AnalyticsData | null = null;
  let coachKey = '';
  let coachPeriod = '';
  let snapToPeriod = false;
  let timeline: CoachTimelineDay | null = null;
  let selectedDay = '';
  let loadedDay = '';
  let selectedSessionKey = '';
  let loadedSessionKey = '';
  let sessionDetail: SessionDetailView | null = null;
  let coachView: 'report' | 'findings' | 'output' | 'activity' = 'report';
  let activityView: 'hours' | 'calendar' | 'projects' = 'hours';
  let findingFilter: 'all' | 'high' | 'medium' | 'low' = 'all';
  let selectedFindingId = '';
  let outputMode: 'language' | 'model' | 'project' = 'language';
  let selectedOutputKey = '';
  let selectedCallIndex = 0;

  $: coachCopy = snapshot.copy.coach;
  $: weekdays = (snapshot.copy.export.calendar_weekdays as string[]) ?? [];
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
      // A period switch re-anchors the selected day; other reloads (background
      // refresh, currency) keep an explicitly chosen context day.
      if (snapshot.period !== coachPeriod) {
        coachPeriod = snapshot.period;
        snapToPeriod = true;
      }
      void loadCoach();
      void loadAnalytics();
    }
  }
  $: if (
    coachView === 'activity' &&
    activityView === 'calendar' &&
    coach &&
    selectedDay &&
    selectedDay !== loadedDay
  ) {
    loadedDay = selectedDay;
    void loadTimeline(selectedDay);
  }
  $: if (
    coachView === 'activity' &&
    selectedSessionKey &&
    selectedSessionKey !== loadedSessionKey
  ) {
    loadedSessionKey = selectedSessionKey;
    void loadSessionDetail(selectedSessionKey);
  }

  async function loadCoach() {
    try {
      const data = await api.getCoach(snapshot.period);
      coach = data;
      const gridDays = data.timeline_grid;
      const current = gridDays.find((d) => d.day === selectedDay);
      if (!current || (snapToPeriod && !current.in_period)) {
        // Prefer the newest in-period day; fall back to the newest context day.
        const inPeriod = gridDays.filter((d) => d.in_period);
        selectedDay = (inPeriod[inPeriod.length - 1] ?? gridDays[gridDays.length - 1])?.day ?? '';
        loadedDay = '';
        timeline = null;
      } else {
        loadedDay = '';
      }
      snapToPeriod = false;
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  async function loadAnalytics() {
    try {
      analytics = await api.getAnalytics(snapshot.period);
    } catch {
      // Keep the previous render on transient errors.
    }
  }

  async function loadTimeline(day: string) {
    timeline = null;
    sessionDetail = null;
    selectedCallIndex = 0;
    loadedSessionKey = '';
    try {
      const data = await api.getCoachTimeline(day);
      if (day !== selectedDay) return;
      timeline = data;
      const rows = data?.rows ?? [];
      if (!rows.some((row) => row.session_key === selectedSessionKey)) {
        selectedSessionKey = rows[0]?.session_key ?? '';
      }
    } catch {
      timeline = null;
    }
  }

  async function loadSessionDetail(key: string) {
    try {
      const data = await api.getSessionDetail(key);
      if (key === selectedSessionKey) {
        sessionDetail = data;
        selectedCallIndex = 0;
      }
    } catch {
      if (key === selectedSessionKey) sessionDetail = null;
    }
  }

  function selectSession(key: string) {
    if (key === selectedSessionKey) return;
    selectedSessionKey = key;
    sessionDetail = null;
    selectedCallIndex = 0;
  }

  function openFinding(ruleId: string) {
    findingFilter = 'all';
    selectedFindingId = ruleId;
    coachView = 'findings';
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

  function gradeLabel(gradeId: string): string {
    return coachCopy.report.grade_labels[gradeId] ?? gradeId;
  }

  // High score is good — same thresholds as ScoreBar. Used for flow days.
  function scoreTone(score: number): 'good' | 'warn' | 'bad' {
    return score >= 70 ? 'good' : score >= 40 ? 'warn' : 'bad';
  }

  // Grade-tier tones: green is earned by A-tier only, B/C read as
  // needs-attention, D/F as trouble.
  function gradeTone(score: number): 'good' | 'warn' | 'bad' {
    return score >= 90 ? 'good' : score >= 70 ? 'warn' : 'bad';
  }

  function severityTone(id: string): 'good' | 'warn' | 'bad' {
    return id === 'high' ? 'bad' : id === 'medium' ? 'warn' : 'good';
  }

  function outputRows(data: CoachData, mode: typeof outputMode): CountMetric[] {
    if (mode === 'model') return data.output.by_model;
    if (mode === 'project') return data.output.by_project;
    return data.output.by_language;
  }

  function dayWeekday(day: string): string {
    const [y, m, d] = day.split('-').map(Number);
    if (!y || !m || !d) return '';
    return new Date(y, m - 1, d).toLocaleDateString(undefined, { weekday: 'long' });
  }

  function shortTime(timestamp: string): string {
    const parsed = new Date(timestamp);
    if (!Number.isNaN(parsed.getTime())) {
      return parsed.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    }
    return timestamp.slice(11, 16) || timestamp;
  }

  function outputTickLabel(name: string): string {
    if (snapshot.period === 'today') return name.slice(11);
    if (snapshot.period === 'week') return `${name.slice(5, 10)} ${name.slice(11, 13)}h`;
    if (snapshot.period === 'all-time') return name;
    return name.slice(5);
  }

  $: totalRules = coach ? coach.practice_groups.reduce((sum, g) => sum + g.total_rules, 0) : 0;
  $: triggeredRules = coach ? coach.practice_groups.reduce((sum, g) => sum + g.triggered, 0) : 0;
  $: rulesClean = `${totalRules - triggeredRules}/${totalRules}`;
  $: findingsCount = coach ? String(coach.findings.length) : '0';
  $: highSeverity = coach
    ? tpl(coachCopy.hero.high_severity, {
        count: coach.findings.filter((f) => f.severity === 'high').length
      })
    : '';
  $: topTips = coach ? coach.findings.slice(0, 3) : [];
  $: findingOccurrences = coach
    ? coach.findings.reduce((sum, finding) => sum + finding.occurrences, 0)
    : 0;
  $: affectedGroups = coach ? new Set(coach.findings.map((finding) => finding.group)).size : 0;
  $: flowTrend = coach ? [...coach.flow.days].reverse().map((day) => day.score) : [];
  $: filteredFindings = coach
    ? coach.findings.filter((finding) => findingFilter === 'all' || finding.severity === findingFilter)
    : [];
  $: if (filteredFindings.length && !filteredFindings.some((finding) => finding.rule_id === selectedFindingId)) {
    selectedFindingId = filteredFindings[0].rule_id;
  }
  $: selectedFinding = filteredFindings.find((finding) => finding.rule_id === selectedFindingId) ?? null;
  $: activeOutputRows = coach ? outputRows(coach, outputMode) : [];
  $: activeOutputTotal = activeOutputRows.reduce((sum, row) => sum + row.calls, 0);
  $: if (activeOutputRows.length && !activeOutputRows.some((row) => row.name === selectedOutputKey)) {
    selectedOutputKey = activeOutputRows[0].name;
  }
  $: selectedOutput = activeOutputRows.find((row) => row.name === selectedOutputKey) ?? null;
  $: selectedOutputShare =
    selectedOutput && activeOutputTotal
      ? `${Math.round((selectedOutput.calls / activeOutputTotal) * 1000) / 10}%`
      : '0%';
  $: outputDays = coach?.output.by_day ?? [];
  $: outputTrendHeading = snapshot.period === 'today'
    ? coachCopy.output.by_half_hour
    : snapshot.period === 'week'
      ? coachCopy.output.by_hour
      : snapshot.period === 'all-time'
        ? coachCopy.output.by_month
        : coachCopy.output.by_day;
  $: outputTrendHint = snapshot.period === 'today'
    ? coachCopy.output.half_hour_hint
    : snapshot.period === 'week'
      ? coachCopy.output.hourly_hint
      : snapshot.period === 'all-time'
        ? coachCopy.output.monthly_hint
        : coachCopy.output.daily_hint;
  $: outputBarsLabel = snapshot.period === 'today'
    ? coachCopy.output.half_hour_output
    : snapshot.period === 'week'
      ? coachCopy.output.hourly_output
      : snapshot.period === 'all-time'
        ? coachCopy.output.monthly_output
        : coachCopy.output.daily_output;
  $: outputAverageLabel = snapshot.period === 'today'
    ? coachCopy.output.half_hour_average
    : snapshot.period === 'week'
      ? coachCopy.output.hourly_average
      : snapshot.period === 'all-time'
        ? coachCopy.output.monthly_average
        : coachCopy.output.moving_average;
  $: outputDailyAverage = outputDays.length
    ? Math.round(outputDays.reduce((sum, row) => sum + row.calls, 0) / outputDays.length)
    : 0;
  $: outputPeak = outputDays.reduce<CountMetric | null>(
    (peak, row) => (!peak || row.calls > peak.calls ? row : peak),
    null
  );
  $: topLanguage = coach?.output.by_language[0] ?? null;
  $: topModel = coach?.output.by_model[0] ?? null;
  $: languageTotal = coach?.output.by_language.reduce((sum, row) => sum + row.calls, 0) ?? 0;
  $: modelTotal = coach?.output.by_model.reduce((sum, row) => sum + row.calls, 0) ?? 0;
  $: topLanguageShare = topLanguage && languageTotal
    ? `${Math.round((topLanguage.calls / languageTotal) * 1000) / 10}%`
    : '0%';
  $: topModelShare = topModel && modelTotal
    ? `${Math.round((topModel.calls / modelTotal) * 1000) / 10}%`
    : '0%';
  $: selectedCall = sessionDetail?.calls[selectedCallIndex] ?? null;
  $: activityProjects = coach
    ? snapshot.dashboard.projects.map((project) => {
        const tools = snapshot.dashboard.project_tools.filter((row) => row.project === project.name);
        const calls = tools.reduce((sum, row) => sum + row.calls, 0);
        const output = coach?.output.by_project.find((row) => row.name === project.name)?.calls ?? 0;
        return { ...project, calls, output, tools: tools.slice(0, 4) };
      })
    : [];
</script>

<section class="page-flow" use:staggeredReveal={{ selector: ':scope > *', y: 5, stagger: 0.035 }}>
  {#if coach}
    <div class="coach-view-tabs" role="tablist" aria-label={coachCopy.tabs.label}>
      <button type="button" role="tab" class:active={coachView === 'report'} aria-selected={coachView === 'report'} onclick={() => (coachView = 'report')}><FileText size={15} aria-hidden="true" /><span>{coachCopy.tabs.report}</span></button>
      <button type="button" role="tab" class:active={coachView === 'findings'} aria-selected={coachView === 'findings'} onclick={() => (coachView = 'findings')}><Search size={15} aria-hidden="true" /><span>{coachCopy.tabs.findings}</span></button>
      <button type="button" role="tab" class:active={coachView === 'output'} aria-selected={coachView === 'output'} onclick={() => (coachView = 'output')}><Code2 size={15} aria-hidden="true" /><span>{coachCopy.tabs.output}</span></button>
      <button type="button" role="tab" class:active={coachView === 'activity'} aria-selected={coachView === 'activity'} onclick={() => (coachView = 'activity')}><CalendarDays size={15} aria-hidden="true" /><span>{coachCopy.tabs.activity}</span></button>
    </div>

    {#if coachView === 'report'}
      <section class="kpis hero-kpis coach-hero">
      <div class="grade-tile">
        <RadialGauge
          score={coach.overall.score}
          label={gradeLabel(coach.overall.grade_id)}
          sublabel={`${coach.overall.score}/100`}
          ariaLabel={coachCopy.report.overall}
        />
        <span>{coachCopy.report.overall}</span>
      </div>
      <div>
        <span>{coachCopy.hero.rules_clean}</span>
        <strong use:countUp={rulesClean}>{rulesClean}</strong>
      </div>
      <div>
        <span>{coachCopy.hero.findings}</span>
        <strong use:countUp={findingsCount}>{findingsCount}</strong>
        <small>{highSeverity}</small>
      </div>
      <div>
        <span>{coachCopy.hero.streak}</span>
        <strong use:countUp={String(coach.pace.current_streak)}>{coach.pace.current_streak}</strong>
        <small>{coachCopy.pace.days_suffix}</small>
      </div>
      <div>
        <span>{coachCopy.hero.total_loc}</span>
        <strong use:countUp={coach.output.total_loc}>{coach.output.total_loc}</strong>
      </div>
      </section>

      <section class="score-grid">
        {#each coach.practice_groups as group (group.id)}
          <Panel title={coachCopy.groups[group.id] ?? group.id} tone="blue">
            <div class="score-card">
            <div class="score-top">
              <span class="score-value mono">{group.score}</span>
              <Badge label={gradeLabel(group.grade_id)} tone={gradeTone(group.score)} />
            </div>
            <ScoreBar score={group.score} ariaLabel={coachCopy.groups[group.id] ?? group.id} />
            <Sparkline
              values={group.trend}
              ariaLabel={`${coachCopy.groups[group.id] ?? group.id}: ${coachCopy.score.trend}`}
            />
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
    {/if}

    {#if coachView === 'report'}
      <Panel title={coachCopy.findings.priority} tone="red">
        {#if topTips.length === 0}
          <p class="coach-empty">{coachCopy.findings.empty}</p>
        {:else}
          <div class="findings-priority">
            {#each topTips as tip (tip.rule_id)}
              <button
                type="button"
                class={`tip ${severityTone(tip.severity)}`}
                onclick={() => openFinding(tip.rule_id)}
              >
                <div class="tip-head">
                  <span class="tip-icon" aria-hidden="true"><Lightbulb size={16} /></span>
                  <strong class="tip-name">{ruleCopy(tip.rule_id).name}</strong>
                  <Badge label={coachCopy.findings.severity[tip.severity] ?? tip.severity} tone={severityTone(tip.severity)} />
                  <ChevronRight class="tip-link-icon" size={15} aria-hidden="true" />
                </div>
                <p class="tip-body">{ruleCopy(tip.rule_id).how_to_improve}</p>
                <div class="tip-meta">
                  <span class="dim">{coachCopy.groups[tip.group] ?? tip.group}</span>
                  <span class="mono tip-count">{tpl(coachCopy.findings.occurrences, { count: count(tip.occurrences) })}</span>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </Panel>

      <section class="duo-grid coach-health-grid">
        <Panel title={coachCopy.flow.title} tone="cyan">
          {#if coach.flow.total_days === 0}
            <p class="coach-empty">{coachCopy.flow.empty}</p>
          {:else}
            <div class="flow-summary">
              <div class="flow-score">
                <span class="mono flow-score-value">{coach.flow.overall_score}</span>
                <Badge label={coachCopy.flow.labels[coach.flow.label_id] ?? coach.flow.label_id} tone={scoreTone(coach.flow.overall_score)} />
              </div>
              <div class="flow-kpis">
                <StatTile value={coach.flow.avg_followup} label={coachCopy.flow.avg_followup} />
                <StatTile value={coach.flow.avg_block} label={coachCopy.flow.avg_block} />
                <StatTile value={`${coach.flow.deep_days}/${coach.flow.total_days}`} label={coachCopy.flow.deep_days} />
                <StatTile value={`${coach.flow.fragmented_days}/${coach.flow.total_days}`} label={coachCopy.flow.fragmented_days} />
              </div>
            </div>
            <div class="flow-trend">
              <span class="stat-label">{coachCopy.flow.trend}</span>
              <Sparkline values={flowTrend} ariaLabel={coachCopy.flow.chart_aria} height={44} />
            </div>
          {/if}
        </Panel>

        <Panel title={coachCopy.pace.title} tone="green">
          <div class="stat-row stat-row-pace">
            <StatTile value={String(coach.pace.current_streak)} label={coachCopy.pace.streak} sub={coachCopy.pace.days_suffix} />
            <StatTile value={String(coach.pace.longest_streak)} label={coachCopy.pace.longest_streak} sub={coachCopy.pace.days_suffix} />
          </div>
          <div class="pace-gauges">
            <div class="pace-gauge">
              <div class="pace-gauge-head"><span class="stat-label">{coachCopy.pace.late_night}</span><span class="mono">{coach.pace.late_night_pct}%</span></div>
              <GaugeBar used={coach.pace.late_night_pct} ariaLabel={coachCopy.pace.late_night} usedSuffix={snapshot.copy.usage.used_suffix} />
            </div>
            <div class="pace-gauge">
              <div class="pace-gauge-head"><span class="stat-label">{coachCopy.pace.weekend}</span><span class="mono">{coach.pace.weekend_pct}%</span></div>
              <GaugeBar used={coach.pace.weekend_pct} ariaLabel={coachCopy.pace.weekend} usedSuffix={snapshot.copy.usage.used_suffix} />
            </div>
          </div>
          <div class="pace-risk"><span class="stat-label">{coachCopy.pace.risk}:</span><Badge label={coachCopy.pace.risks[coach.pace.risk_id] ?? coach.pace.risk_id} tone={severityTone(coach.pace.risk_id)} /></div>
          {#if coach.pace.alert_ids.length > 0}
            <ul class="pace-alerts">
              {#each coach.pace.alert_ids as alert (alert)}
                <li><span class="alert-icon" aria-hidden="true"><TriangleAlert size={14} /></span>{coachCopy.pace.alerts[alert] ?? alert}</li>
              {/each}
            </ul>
          {/if}
        </Panel>
      </section>
    {:else if coachView === 'findings'}
      <section class="kpis finding-kpis">
        <div><span>{coachCopy.findings.high}</span><strong use:countUp={String(coach.findings.filter((finding) => finding.severity === 'high').length)}>{coach.findings.filter((finding) => finding.severity === 'high').length}</strong></div>
        <div><span>{coachCopy.findings.medium}</span><strong use:countUp={String(coach.findings.filter((finding) => finding.severity === 'medium').length)}>{coach.findings.filter((finding) => finding.severity === 'medium').length}</strong></div>
        <div><span>{coachCopy.findings.affected_practices}</span><strong use:countUp={String(affectedGroups)}>{affectedGroups}</strong></div>
        <div><span>{coachCopy.findings.total_occurrences}</span><strong use:countUp={count(findingOccurrences)}>{count(findingOccurrences)}</strong></div>
      </section>
      <Panel title={coachCopy.findings.all} tone="red">
        {#if coach.findings.length === 0}
          <p class="coach-empty">{coachCopy.findings.empty}</p>
        {:else}
          <div class="finding-explorer">
            <aside class="finding-master">
              <div class="segmented finding-filters" role="tablist" aria-label={coachCopy.findings.filter_label}>
                <button type="button" role="tab" class:active={findingFilter === 'all'} aria-selected={findingFilter === 'all'} onclick={() => (findingFilter = 'all')}>{snapshot.copy.filters.all}</button>
                <button type="button" role="tab" class:active={findingFilter === 'high'} aria-selected={findingFilter === 'high'} onclick={() => (findingFilter = 'high')}>{coachCopy.findings.severity.high}</button>
                <button type="button" role="tab" class:active={findingFilter === 'medium'} aria-selected={findingFilter === 'medium'} onclick={() => (findingFilter = 'medium')}>{coachCopy.findings.severity.medium}</button>
                <button type="button" role="tab" class:active={findingFilter === 'low'} aria-selected={findingFilter === 'low'} onclick={() => (findingFilter = 'low')}>{coachCopy.findings.severity.low}</button>
              </div>
              <ul class="finding-list">
                {#each filteredFindings as finding (finding.rule_id)}
                  <li>
                    <button type="button" class={`finding-row ${severityTone(finding.severity)}`} class:selected={finding.rule_id === selectedFindingId} onclick={() => (selectedFindingId = finding.rule_id)}>
                      <span class="finding-severity">
                        <Badge label={coachCopy.findings.severity[finding.severity] ?? finding.severity} tone={severityTone(finding.severity)} />
                      </span>
                      <span class="finding-copy">
                        <span class="finding-name">{ruleCopy(finding.rule_id).name}</span>
                        <span class="finding-group dim">{coachCopy.groups[finding.group] ?? finding.group}</span>
                      </span>
                      <span class="finding-count mono">{tpl(coachCopy.findings.occurrences, { count: count(finding.occurrences) })}</span>
                      <span class="finding-trigger">{findingLine(finding)}</span>
                    </button>
                  </li>
                {/each}
              </ul>
            </aside>
            {#if selectedFinding}
              <article class="finding-detail">
                <header class="finding-detail-head">
                  <div>
                    <span class="stat-label">{coachCopy.findings.selected}</span>
                    <h3>{ruleCopy(selectedFinding.rule_id).name}</h3>
                    <span class="dim">{coachCopy.groups[selectedFinding.group] ?? selectedFinding.group}</span>
                  </div>
                  <Badge label={coachCopy.findings.severity[selectedFinding.severity] ?? selectedFinding.severity} tone={severityTone(selectedFinding.severity)} />
                </header>
                <div class="finding-detail-kpis">
                  <StatTile value={count(selectedFinding.occurrences)} label={coachCopy.findings.occurrences_label} />
                  <StatTile value={count(selectedFinding.total)} label={coachCopy.findings.sample_size} />
                  <StatTile value={selectedFinding.pct || '—'} label={coachCopy.findings.trigger_rate} />
                  <StatTile value={String(selectedFinding.examples.length)} label={coachCopy.findings.evidence_count} />
                </div>
                <section class="finding-detail-section">
                  <h4>{coachCopy.findings.why_it_matters}</h4>
                  <p>{ruleCopy(selectedFinding.rule_id).description}</p>
                  <p class="finding-signal">{findingLine(selectedFinding)}</p>
                </section>
                <section class="finding-detail-section finding-next-move">
                  <h4>{coachCopy.findings.improve}</h4>
                  <p>{ruleCopy(selectedFinding.rule_id).how_to_improve}</p>
                </section>
                <section class="finding-detail-section">
                  <h4>{coachCopy.findings.examples}</h4>
                  {#if selectedFinding.examples.length}
                    <ol class="evidence-list">
                      {#each selectedFinding.examples as example, index}
                        <li><span class="mono evidence-index">{String(index + 1).padStart(2, '0')}</span><div><p class="mono">"{example.text}"</p>{#if example.detail}<span>{example.detail}</span>{/if}</div></li>
                      {/each}
                    </ol>
                  {:else}
                    <p class="dim">{coachCopy.findings.no_examples}</p>
                  {/if}
                </section>
              </article>
            {/if}
          </div>
        {/if}
      </Panel>
    {:else if coachView === 'output'}
      <Panel title={coachCopy.output.title} tone="magenta">
        {#if coach.output.by_language.length === 0}
          <p class="coach-empty">{coachCopy.output.empty}</p>
        {:else}
          <div class="output-kpis">
            <div class="output-kpi"><strong class="mono">{coach.output.total_loc}</strong><span>{coachCopy.output.total}</span></div>
            <div class="output-kpi"><strong class="mono">{outputDays.length}</strong><span>{coachCopy.output.active_days}</span></div>
            <div class="output-kpi"><strong class="mono">{count(outputDailyAverage)}</strong><span>{coachCopy.output.daily_average}</span></div>
            <div class="output-kpi"><strong class="mono">{outputPeak ? count(outputPeak.calls) : '0'}</strong><span>{coachCopy.output.peak_day}</span>{#if outputPeak}<small>{outputPeak.name}</small>{/if}</div>
            <div class="output-kpi"><strong title={topLanguage?.name ?? ''}>{topLanguage?.name ?? '—'}</strong><span>{coachCopy.output.top_language}</span>{#if topLanguage}<small>{tpl(coachCopy.output.share, { share: topLanguageShare })}</small>{/if}</div>
            <div class="output-kpi"><strong title={topModel?.name ?? ''}>{topModel?.name ?? '—'}</strong><span>{coachCopy.output.top_model}</span>{#if topModel}<small>{tpl(coachCopy.output.share, { share: topModelShare })}</small>{/if}</div>
          </div>
          <div class="output-explorer">
            <section class="output-trend">
              <div class="section-heading">
                <div><h3>{outputTrendHeading}</h3><p>{outputTrendHint}</p></div>
                {#if coach.output.uncovered_tools}<span class="dim coverage-note">{tpl(coachCopy.output.coverage_note, { tools: coach.output.uncovered_tools })}</span>{/if}
              </div>
              <OutputTrend
                rows={coach.output.trend}
                ariaLabel={outputTrendHeading}
                emptyLabel={coachCopy.output.empty}
                barsLabel={outputBarsLabel}
                averageLabel={outputAverageLabel}
                tickLabel={(row) => outputTickLabel(row.name)}
                detailLabel={(row, average) => tpl(coachCopy.output.day_detail, { day: row.name, lines: count(row.calls), average: count(Math.round(average)) })}
              />
            </section>
            <section class="output-breakdown">
              <div class="output-breakdown-head">
                <h3>{coachCopy.output.breakdown}</h3>
                <div class="segmented output-tabs" role="tablist" aria-label={coachCopy.output.breakdown}>
                  <button type="button" role="tab" class:active={outputMode === 'language'} aria-selected={outputMode === 'language'} onclick={() => (outputMode = 'language')}>{coachCopy.output.languages}</button>
                  <button type="button" role="tab" class:active={outputMode === 'model'} aria-selected={outputMode === 'model'} onclick={() => (outputMode = 'model')}>{coachCopy.output.models}</button>
                  <button type="button" role="tab" class:active={outputMode === 'project'} aria-selected={outputMode === 'project'} onclick={() => (outputMode = 'project')}>{coachCopy.output.projects}</button>
                </div>
              </div>
              <ul class="output-list">
                {#each activeOutputRows as row (row.name)}
                  <li><button type="button" class:selected={row.name === selectedOutputKey} onclick={() => (selectedOutputKey = row.name)}><RankBar value={row.value} ariaLabel={row.name} compact /><span class="output-name">{row.name}</span><span class="mono output-count">{count(row.calls)}</span></button></li>
                {/each}
              </ul>
              {#if selectedOutput}<div class="output-selected"><span class="stat-label">{coachCopy.output.selected}</span><strong>{selectedOutput.name}</strong><span class="mono">{count(selectedOutput.calls)} · {selectedOutputShare}</span></div>{/if}
            </section>
          </div>
        {/if}
      </Panel>
    {:else}
      <div class="activity-view-tabs segmented" role="tablist" aria-label={coachCopy.timeline.views_label}>
        <button type="button" role="tab" class:active={activityView === 'hours'} aria-selected={activityView === 'hours'} onclick={() => (activityView = 'hours')}>{coachCopy.timeline.work_hours}</button>
        <button type="button" role="tab" class:active={activityView === 'calendar'} aria-selected={activityView === 'calendar'} onclick={() => (activityView = 'calendar')}>{coachCopy.timeline.calendar_view}</button>
        <button type="button" role="tab" class:active={activityView === 'projects'} aria-selected={activityView === 'projects'} onclick={() => (activityView = 'projects')}>{coachCopy.timeline.projects_view}</button>
      </div>

      {#if activityView === 'hours'}
        <Panel title={coachCopy.timeline.patterns_title} tone="orange">
          {#if analytics}
            <ActivityHourGrid matrix={analytics.hour_day} dayLabels={weekdays} ariaLabel={coachCopy.timeline.patterns_title} emptyLabel={snapshot.copy.empty.no_data} lessLabel={coachCopy.timeline.less} moreLabel={coachCopy.timeline.more} />
          {:else}
            <p class="coach-empty">{coachCopy.timeline.loading_patterns}</p>
          {/if}
        </Panel>
        <section class="duo-grid activity-pattern-charts">
          <Panel title={coachCopy.timeline.hourly_profile} tone="cyan">
            {#if analytics}
              <ActivityProfile matrix={analytics.hour_day} ariaLabel={coachCopy.timeline.hourly_profile} weekdayLabel={coachCopy.timeline.weekdays} weekendLabel={coachCopy.timeline.weekends} emptyLabel={snapshot.copy.empty.no_data} />
            {/if}
          </Panel>
          <Panel title={coachCopy.timeline.period_trend} tone="blue">
            <ActivityPulse points={snapshot.dashboard.activity_timeline} copy={snapshot.copy} />
          </Panel>
        </section>
      {:else if activityView === 'projects'}
        <Panel title={coachCopy.timeline.projects_title} tone="green">
          {#if activityProjects.length}
            <div class="activity-projects">
              {#each activityProjects as project (project.name)}
                <article class="activity-project">
                  <div class="activity-project-head"><strong>{project.name}</strong><span class="mono">{project.cost}</span></div>
                  <RankBar value={project.value} ariaLabel={project.name} />
                  <div class="activity-project-kpis">
                    <span><b class="mono">{count(project.calls)}</b>{snapshot.copy.metrics.calls}</span>
                    <span><b class="mono">{project.sessions}</b>{snapshot.copy.metrics.sessions}</span>
                    <span><b class="mono">{count(project.output)}</b>{coachCopy.timeline.ai_loc}</span>
                    <span><b class="mono">{project.avg_per_session}</b>{snapshot.copy.tables.avg_per_session}</span>
                  </div>
                  {#if project.tools.length}
                    <div class="project-tools">{#each project.tools as tool (`${project.name}-${tool.tool}`)}<span>{tool.tool}</span>{/each}</div>
                  {/if}
                </article>
              {/each}
            </div>
          {:else}
            <p class="coach-empty">{coachCopy.timeline.no_projects}</p>
          {/if}
        </Panel>
      {:else}
        <Panel title={coachCopy.timeline.calendar} tone="orange">
          {#if coach.timeline_grid.length === 0}
            <p class="coach-empty">{coachCopy.timeline.empty}</p>
          {:else}
            <div class="timeline-overview">
              <CalendarBars days={coach.timeline_grid} selected={selectedDay} ariaLabel={coachCopy.timeline.calendar} emptyLabel={coachCopy.timeline.empty} barsLabel={coachCopy.timeline.bars_label} averageLabel={coachCopy.timeline.average_label} detailLabel={(day, turns) => `${day} · ${tpl(coachCopy.timeline.turns, { count: count(turns) })}`} onSelect={(day) => (selectedDay = day)} />
              <div class="day-summary">
                <span class="stat-label">{coachCopy.timeline.day}</span><strong class="mono day-value">{selectedDay}</strong><span class="day-weekday">{dayWeekday(selectedDay)}</span>
                {#if timeline}<div class="day-facts mono"><span>{tpl(coachCopy.timeline.sessions, { count: timeline.rows.length })}</span><span>{tpl(coachCopy.timeline.turns, { count: count(timeline.rows.reduce((sum, row) => sum + row.turns, 0)) })}</span>{#if timeline.total_cost}<span class="day-cost">{timeline.total_cost}</span>{/if}{#if timeline.max_concurrent > 1}<span class="overlap-badge">{tpl(coachCopy.timeline.overlap, { count: timeline.max_concurrent })}</span>{/if}</div>{/if}
                <p>{coachCopy.timeline.calendar_hint}</p>
              </div>
            </div>
            {#if timeline}
              <section class="session-lanes calendar-session-lanes">
                <div class="section-heading"><div><h3>{coachCopy.timeline.session_activity}</h3><p>{coachCopy.timeline.session_hint}</p></div></div>
                <SessionTimeline rows={timeline.rows} windowStartMin={timeline.window_start_min} windowEndMin={timeline.window_end_min} headers={{ time: snapshot.copy.tables.time, project: snapshot.copy.tables.project, tool: snapshot.copy.tables.tool, turns: coachCopy.timeline.turns_header, cost: snapshot.copy.tables.cost }} ariaLabel={coachCopy.timeline.title} emptyLabel={coachCopy.timeline.empty} selectedKey={selectedSessionKey} onSelect={selectSession} />
              </section>
              {#if sessionDetail}
                <section class="calendar-session-detail">
                  <div class="session-inspector-head"><div><span class="stat-label">{snapshot.copy.panels.selected_session}</span><strong>{sessionDetail.project}</strong><span class="dim">{sessionDetail.tool} · {sessionDetail.date_range}</span></div><span class="mono session-cost">{sessionDetail.total_cost}</span></div>
                  <div class="session-facts"><StatTile value={String(sessionDetail.total_calls)} label={snapshot.copy.metrics.calls} /><StatTile value={sessionDetail.total_input} label={snapshot.copy.metrics.input} /><StatTile value={sessionDetail.total_output} label={snapshot.copy.metrics.output} /><StatTile value={sessionDetail.total_cache_read} label={snapshot.copy.metrics.cache_read} /></div>
                  <div class="request-workspace calendar-request-workspace">
                    <div class="request-list" role="listbox" aria-label={snapshot.copy.panels.calls}>
                      {#each sessionDetail.calls as call, index (`${call.timestamp}-${index}`)}
                        <button type="button" class="request-row" class:selected={index === selectedCallIndex} role="option" aria-selected={index === selectedCallIndex} onclick={() => (selectedCallIndex = index)}><span class="mono request-time">{shortTime(call.timestamp)}</span><span class="request-model">{call.model}</span><span class="request-prompt">{call.prompt}</span><span class="mono request-cost">{call.cost}</span></button>
                      {/each}
                    </div>
                    {#if selectedCall}<aside class="call-inspector"><div class="call-inspector-head"><div><span class="stat-label">{coachCopy.timeline.selected_call}</span><strong>{selectedCall.model}</strong></div><span class="mono session-cost">{selectedCall.cost}</span></div><p class="call-prompt">{selectedCall.prompt_full || selectedCall.prompt}</p><div class="call-token-grid"><StatTile value={count(selectedCall.input_tokens)} label={snapshot.copy.metrics.input} /><StatTile value={count(selectedCall.output_tokens)} label={snapshot.copy.metrics.output} /><StatTile value={count(selectedCall.cache_read + selectedCall.cache_write)} label={snapshot.copy.metrics.cache} /></div><div class="call-meta"><span><b>{snapshot.copy.tables.time}</b><span class="mono">{shortTime(selectedCall.timestamp)}</span></span><span><b>{snapshot.copy.tables.tools}</b><span>{selectedCall.tools || coachCopy.timeline.no_tools}</span></span></div></aside>{/if}
                  </div>
                </section>
              {:else}
                <p class="coach-empty calendar-loading">{coachCopy.timeline.loading}</p>
              {/if}
            {:else}
              <p class="coach-empty calendar-loading">{coachCopy.timeline.loading}</p>
            {/if}
          {/if}
        </Panel>
      {/if}
    {/if}
  {/if}
</section>

<style>
  .coach-hero {
    grid-template-columns: minmax(230px, 1.4fr) repeat(4, minmax(0, 1fr));
  }

  .coach-hero .grade-tile {
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-start;
    gap: var(--space-xl);
    padding-left: 22px;
  }

  @media (max-width: 1100px) {
    .coach-hero {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .coach-hero .grade-tile {
      grid-column: 1 / -1;
    }
  }

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

  .score-top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
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

  .coach-view-tabs {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    align-items: end;
    gap: 2px;
    width: 100%;
    padding: 6px 6px 0;
    background: var(--color-surface-sunken);
    border-bottom: 1px solid var(--color-border-soft);
  }

  .coach-view-tabs button {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-md);
    min-width: 0;
    min-height: 40px;
    padding: 8px var(--space-xl);
    color: var(--color-muted-2);
    background: transparent;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    font-family: var(--font-ui);
    font-size: var(--text-body);
    font-weight: 600;
  }

  .coach-view-tabs button :global(svg) {
    flex: 0 0 auto;
    opacity: 0.72;
  }

  .coach-view-tabs button:hover {
    color: var(--color-on-surface);
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
    border-color: transparent;
  }

  .coach-view-tabs button.active {
    z-index: 1;
    margin-bottom: -1px;
    color: var(--color-primary);
    background: var(--color-neutral);
    border-color: var(--color-border-soft);
  }

  .coach-view-tabs button.active::after {
    content: "";
    position: absolute;
    right: 0;
    bottom: -1px;
    left: 0;
    height: 1px;
    background: var(--color-neutral);
  }

  .coach-view-tabs button.active :global(svg) {
    opacity: 1;
  }

  .findings-priority {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: var(--space-lg);
  }

  .coach-health-grid {
    align-items: stretch;
  }

  .coach-health-grid :global(.panel) {
    height: 100%;
  }

  .finding-kpis {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .output-kpis {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    background: var(--color-neutral);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-md);
  }

  .output-kpi {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: var(--space-xl);
    border-right: 1px solid var(--color-border-row);
  }

  .output-kpi:last-child {
    border-right: 0;
  }

  .output-kpi strong {
    color: var(--color-on-surface);
    font-size: var(--text-display);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-kpi span,
  .output-kpi small {
    color: var(--color-muted-2);
    font-size: var(--text-label);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-kpi small {
    color: var(--color-muted);
  }

  .tip {
    position: relative;
    width: 100%;
    padding: 12px 14px 12px 18px;
    background: transparent;
    border: 0;
    border-right: 1px solid var(--color-border-row);
    border-radius: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    gap: 6px;
    color: inherit;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
  }

  .tip:last-child {
    border-right: 0;
  }

  .tip::before {
    content: '';
    position: absolute;
    left: 0;
    top: 12px;
    bottom: 12px;
    width: 3px;
    border-radius: 999px;
    background: var(--color-muted-2);
  }

  .tip.bad::before {
    background: var(--color-error);
  }

  .tip.warn::before {
    background: var(--color-warning);
  }

  .tip.good::before {
    background: var(--color-tertiary);
  }

  .tip:hover {
    color: inherit;
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
  }

  .tip:focus-visible {
    outline: 1px solid var(--color-secondary);
    outline-offset: -1px;
  }

  .tip-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tip-icon {
    display: grid;
    place-items: center;
    color: var(--color-warning);
    flex: 0 0 auto;
  }

  .tip-name {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--text-body);
    color: var(--color-on-surface);
  }

  .tip-link-icon {
    flex: 0 0 auto;
    color: var(--color-muted-2);
    transition:
      color var(--motion-fast) var(--ease-standard),
      transform var(--motion-fast) var(--ease-standard);
  }

  .tip:hover :global(.tip-link-icon) {
    color: var(--color-primary);
    transform: translateX(2px);
  }

  .tip-body {
    margin: 0;
    font-size: var(--text-body);
    color: var(--color-muted);
  }

  .tip-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-label);
  }

  .tip-count {
    margin-left: auto;
    color: var(--color-muted);
  }

  .finding-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 540px;
    overflow-y: auto;
  }

  .finding-explorer {
    display: grid;
    grid-template-columns: minmax(300px, 0.72fr) minmax(0, 1.28fr);
    min-height: 540px;
  }

  .finding-master {
    min-width: 0;
    padding-right: var(--space-xl);
    border-right: 1px solid var(--color-border-row);
  }

  .finding-filters {
    width: 100%;
    margin-bottom: var(--space-lg);
  }

  .finding-filters button {
    min-width: 0;
    flex: 1 1 0;
    padding-inline: 8px;
  }

  .finding-list li {
    border-bottom: 1px solid var(--color-border-row);
  }

  .finding-row {
    position: relative;
    width: 100%;
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr) auto;
    align-items: start;
    justify-content: stretch;
    gap: 5px 10px;
    padding: var(--space-lg) var(--space-lg) var(--space-lg) calc(var(--space-lg) + 5px);
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .finding-row::before {
    content: '';
    position: absolute;
    inset: var(--space-lg) auto var(--space-lg) 0;
    width: 3px;
    border-radius: var(--radius-pill);
    background: var(--color-muted-2);
  }

  .finding-row.bad::before {
    background: var(--color-error);
  }

  .finding-row.warn::before {
    background: var(--color-warning);
  }

  .finding-row.good::before {
    background: var(--color-tertiary);
  }

  .finding-row:hover {
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
  }

  .finding-row.selected {
    background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  }

  .finding-row:focus-visible {
    outline: 1px solid var(--color-secondary);
    outline-offset: -1px;
  }

  .finding-severity {
    display: flex;
    align-items: flex-start;
    justify-content: flex-start;
    min-width: 0;
    padding-top: 1px;
  }

  .finding-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 0;
    text-align: left;
  }

  .finding-trigger {
    grid-column: 2 / -1;
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.35;
    text-align: left;
  }

  .finding-name {
    color: var(--color-on-surface);
    font-size: var(--text-body);
    font-weight: 600;
    line-height: 1.3;
  }

  .finding-group {
    font-size: var(--text-label);
  }

  .finding-count {
    justify-self: end;
    padding-top: 1px;
    font-size: var(--text-label);
    color: var(--color-muted);
    white-space: nowrap;
  }

  .finding-detail {
    min-width: 0;
    padding-left: var(--space-2xl);
  }

  .finding-detail-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-xl);
    padding-bottom: var(--space-xl);
  }

  .finding-detail-head > div {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .finding-detail-head h3 {
    margin: 0;
    color: var(--color-on-surface);
    font-size: var(--text-display-lg);
    font-weight: 600;
  }

  .finding-detail-kpis {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-xl);
    padding: var(--space-xl) 0;
    border-top: 1px solid var(--color-border-row);
    border-bottom: 1px solid var(--color-border-row);
  }

  .finding-detail-section {
    padding: var(--space-xl) 0;
    border-top: 1px solid var(--color-border-row);
  }

  .finding-detail-kpis + .finding-detail-section {
    border-top: 0;
  }

  .finding-detail-section h4 {
    margin: 0 0 6px;
    color: var(--color-on-surface);
    font-size: var(--text-label);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .finding-detail-section p {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 1.45;
  }

  .finding-signal {
    margin-top: 8px !important;
    color: var(--color-on-surface) !important;
  }

  .finding-next-move {
    padding-left: var(--space-xl);
    border-left: 3px solid var(--color-warning);
  }

  .evidence-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .evidence-list li {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    gap: var(--space-lg);
    padding: var(--space-lg) 0;
    border-top: 1px solid var(--color-border-row);
  }

  .evidence-list li:first-child {
    border-top: 0;
  }

  .evidence-list p {
    color: var(--color-on-surface);
    overflow-wrap: anywhere;
  }

  .evidence-list span {
    color: var(--color-muted-2);
    font-size: var(--text-label);
  }

  .evidence-index {
    padding-top: 1px;
  }

  .flow-summary {
    display: grid;
    grid-template-columns: minmax(100px, 0.7fr) minmax(0, 2.3fr);
    gap: var(--space-xl);
    align-items: center;
  }

  .flow-score {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding-right: var(--space-xl);
    border-right: 1px solid var(--color-border-row);
  }

  .flow-score-value {
    font-size: var(--text-display-xl);
    font-weight: 700;
  }

  .flow-kpis {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-lg) var(--space-xl);
  }

  .flow-trend {
    display: grid;
    grid-template-columns: 92px 1fr;
    align-items: center;
    gap: var(--space-lg);
    margin-top: var(--space-lg);
    padding-top: var(--space-lg);
    border-top: 1px solid var(--color-border-row);
  }

  .stat-row {
    display: grid;
    gap: 8px;
    margin-bottom: 10px;
  }

  .stat-row-pace {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .stat-label {
    font-size: var(--text-label);
    color: var(--color-muted-2);
  }

  .pace-gauges {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-lg);
    margin-bottom: 10px;
  }

  .pace-gauge {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .pace-gauge-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--text-label);
  }

  .pace-risk {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .pace-alerts {
    list-style: none;
    margin: 4px 0 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--text-label);
    color: var(--color-warning);
  }

  .pace-alerts li {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .alert-icon {
    display: grid;
    place-items: center;
    flex: 0 0 auto;
  }

  .section-heading,
  .output-breakdown-head,
  .session-inspector-head,
  .request-feed-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-lg);
  }

  .section-heading h3,
  .output-breakdown-head h3 {
    margin: 0;
    font-size: var(--text-heading);
    font-weight: 600;
    color: var(--color-on-surface);
  }

  .section-heading p,
  .day-summary p {
    margin: 2px 0 0;
    font-size: var(--text-label);
    color: var(--color-muted-2);
  }

  .timeline-overview {
    display: grid;
    grid-template-columns: minmax(480px, 1fr) minmax(220px, 300px);
    gap: var(--space-2xl);
    align-items: start;
    padding-bottom: var(--space-xl);
    border-bottom: 1px solid var(--color-border-row);
  }

  .day-summary {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    padding-top: 14px;
  }

  .day-value {
    font-size: var(--text-display-lg);
    font-weight: 600;
  }

  .day-weekday {
    font-size: var(--text-label);
    color: var(--color-muted-2);
  }

  .day-cost {
    color: var(--color-warning);
  }

  .day-facts {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--text-label);
    color: var(--color-muted);
    flex-wrap: wrap;
  }

  .overlap-badge {
    font-size: var(--text-label);
    color: var(--color-warning);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-pill);
    padding: 1px 8px;
  }

  .timeline-workspace {
    display: grid;
    grid-template-columns: minmax(390px, 0.75fr) minmax(600px, 1.25fr);
    gap: var(--space-2xl);
    margin-top: var(--space-xl);
    min-height: 360px;
  }

  .session-lanes,
  .session-inspector,
  .output-trend,
  .output-breakdown {
    min-width: 0;
  }

  .session-lanes {
    display: flex;
    flex-direction: column;
    gap: var(--space-lg);
  }

  .session-inspector {
    border-left: 1px solid var(--color-border-row);
    padding-left: var(--space-2xl);
  }

  .session-inspector-head > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .session-inspector-head strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-cost {
    color: var(--color-warning);
    font-weight: 600;
  }

  .session-facts {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-lg);
    margin: var(--space-xl) 0;
    padding: var(--space-lg) 0;
    border-top: 1px solid var(--color-border-row);
    border-bottom: 1px solid var(--color-border-row);
  }

  .request-feed {
    min-width: 0;
  }

  .request-feed-head {
    padding-bottom: 6px;
  }

  .request-feed-head span {
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .request-row {
    width: 100%;
    display: grid;
    grid-template-columns: 48px minmax(80px, 0.75fr) minmax(130px, 1.5fr) auto;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    border: 0;
    border-top: 1px solid var(--color-border-row);
    border-left: 2px solid transparent;
    background: transparent;
    color: inherit;
    padding: 0 6px;
    cursor: pointer;
    font-size: var(--text-label);
    text-align: left;
  }

  .request-row:hover {
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
  }

  .request-row.selected {
    border-left-color: var(--color-primary);
    background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  }

  .request-row:focus-visible {
    outline: 1px solid var(--color-secondary);
    outline-offset: -1px;
  }

  .request-model,
  .request-prompt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .request-time,
  .request-cost {
    color: var(--color-muted);
  }

  .request-workspace {
    display: grid;
    grid-template-columns: minmax(300px, 1.1fr) minmax(270px, 0.9fr);
    min-height: 260px;
    border-top: 1px solid var(--color-border-row);
  }

  .request-list {
    max-height: 300px;
    overflow-y: auto;
    border-right: 1px solid var(--color-border-row);
  }

  .call-inspector {
    display: flex;
    flex-direction: column;
    gap: var(--space-lg);
    min-width: 0;
    padding: var(--space-lg) 0 var(--space-lg) var(--space-xl);
  }

  .call-inspector-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-lg);
  }

  .call-inspector-head > div {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .call-inspector-head strong {
    overflow: hidden;
    color: var(--color-on-surface);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .call-prompt {
    max-height: 86px;
    margin: 0;
    overflow-y: auto;
    color: var(--color-on-surface);
    font-size: var(--text-body);
    line-height: 1.45;
  }

  .call-token-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-lg);
    padding: var(--space-lg) 0;
    border-top: 1px solid var(--color-border-row);
    border-bottom: 1px solid var(--color-border-row);
  }

  .call-meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--text-label);
  }

  .call-meta > span {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    gap: 8px;
  }

  .call-meta b {
    color: var(--color-muted-2);
    font-weight: 500;
  }

  .call-meta span span {
    color: var(--color-on-surface);
    overflow-wrap: anywhere;
  }

  .output-explorer {
    display: flex;
    flex-direction: column;
    gap: var(--space-xl);
    margin-top: var(--space-2xl);
  }

  .output-trend {
    padding-bottom: var(--space-xl);
    border-bottom: 1px solid var(--color-border-row);
  }

  .output-tabs {
    flex: 0 0 auto;
  }

  .coverage-note {
    font-size: var(--text-label);
    max-width: 300px;
    text-align: right;
  }

  .output-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 2px;
    max-height: 300px;
    overflow-y: auto;
    margin-top: var(--space-lg);
  }

  .activity-view-tabs {
    width: fit-content;
  }

  .activity-view-tabs button {
    min-width: 118px;
  }

  .activity-pattern-charts {
    align-items: stretch;
  }

  .calendar-session-lanes {
    gap: var(--space-lg);
    padding: var(--space-xl) 0;
    border-bottom: 1px solid var(--color-border-row);
  }

  .calendar-session-detail {
    padding-top: var(--space-xl);
  }

  .calendar-request-workspace {
    grid-template-columns: minmax(440px, 1.25fr) minmax(340px, 0.75fr);
  }

  .calendar-loading {
    padding: var(--space-xl) 0;
  }

  .activity-projects {
    display: flex;
    flex-direction: column;
  }

  .activity-project {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: var(--space-xl) 0;
    border-bottom: 1px solid var(--color-border-row);
  }

  .activity-project:first-child {
    padding-top: 0;
  }

  .activity-project:last-child {
    padding-bottom: 0;
    border-bottom: 0;
  }

  .activity-project-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-xl);
  }

  .activity-project-head strong {
    color: var(--color-on-surface);
    font-size: var(--text-heading);
  }

  .activity-project-head span {
    color: var(--color-warning);
  }

  .activity-project-kpis {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--space-xl);
  }

  .activity-project-kpis span {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--color-muted-2);
    font-size: var(--text-label);
  }

  .activity-project-kpis b {
    color: var(--color-on-surface);
    font-size: var(--text-display);
    font-weight: 600;
  }

  .project-tools {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .project-tools span {
    padding: 2px 8px;
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .output-list li {
    font-size: var(--text-label);
  }

  .output-list button {
    width: 100%;
    display: grid;
    grid-template-columns: 86px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    border: 0;
    border-left: 2px solid transparent;
    background: transparent;
    color: inherit;
    padding: 2px 6px;
    text-align: left;
    cursor: pointer;
  }

  .output-list button:hover {
    background: color-mix(in srgb, var(--color-on-surface) 5%, transparent);
  }

  .output-list button.selected {
    border-left-color: var(--color-magenta);
    background: color-mix(in srgb, var(--color-magenta) 8%, transparent);
  }

  .output-name {
    color: var(--color-on-surface);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .output-count {
    color: var(--color-muted);
  }

  .output-selected {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--space-lg);
    margin-top: var(--space-lg);
    padding-top: var(--space-lg);
    border-top: 1px solid var(--color-border-row);
  }

  .output-selected strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 1180px) {
    .timeline-workspace,
    .output-explorer {
      grid-template-columns: 1fr;
    }

    .session-inspector,
    .output-trend {
      border-left: 0;
      border-right: 0;
      padding-left: 0;
      padding-right: 0;
    }

    .session-inspector {
      border-top: 1px solid var(--color-border-row);
      padding-top: var(--space-xl);
    }

    .output-kpis {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .output-kpi:nth-child(3) {
      border-right: 0;
    }

    .output-kpi:nth-child(-n + 3) {
      border-bottom: 1px solid var(--color-border-row);
    }

    .finding-explorer,
    .calendar-request-workspace {
      grid-template-columns: 1fr;
    }

    .finding-master {
      padding-right: 0;
      padding-bottom: var(--space-xl);
      border-right: 0;
      border-bottom: 1px solid var(--color-border-row);
    }

    .finding-detail {
      padding-top: var(--space-xl);
      padding-left: 0;
    }
  }

  @media (max-width: 780px) {
    .finding-kpis,
    .output-kpis,
    .session-facts {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .finding-evidence-grid,
    .request-workspace {
      grid-template-columns: 1fr;
    }

    .finding-detail-kpis,
    .activity-project-kpis {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .output-list {
      grid-template-columns: 1fr;
    }

    .request-list {
      border-right: 0;
      border-bottom: 1px solid var(--color-border-row);
    }

    .call-inspector {
      padding-left: 0;
    }

    .coach-view-tabs {
      grid-template-columns: repeat(4, minmax(96px, 1fr));
      overflow-x: auto;
    }

    .coach-view-tabs button {
      padding-right: var(--space-lg);
      padding-left: var(--space-lg);
    }
  }
</style>
