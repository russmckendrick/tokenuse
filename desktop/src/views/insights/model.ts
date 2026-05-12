import type {
  AdviceDataScopeId,
  AdviceItemStatusId,
  AdviceItemView,
  AdviceRunView,
  CopyDeck,
  DesktopSnapshot,
  RecommendationView
} from '../../types';

export type MainInsightsScreen = 'overview' | 'actions' | 'signals' | 'runs';
export type InsightsScreen = MainInsightsScreen | 'detail';
export type ActionFilter = 'all' | 'open' | 'done' | 'dismissed';
export type InsightSource = 'advice' | 'signal';
export type SeverityId = 'risk' | 'warn' | 'info';
export type InsightStatus = AdviceItemStatusId | 'signal';
export type InsightsCopy = CopyDeck['insights'];

export type InsightRow = {
  id: string;
  source: InsightSource;
  sourceLabel: string;
  severity: SeverityId;
  severityLabel: string;
  category: string;
  categoryId: string;
  title: string;
  body: string;
  impact: string | null;
  savings: string | null;
  savingsUsd: number | null;
  confidence: string | null;
  status: InsightStatus;
  statusLabel: string;
  scopeLabel: string;
  evidence: string[];
  nextStep: string | null;
  assumption: string | null;
  silencedReason: string | null;
  ruleId: string | null;
  run: AdviceRunView | null;
  advice: AdviceItemView | null;
  recommendation: RecommendationView | null;
};

export type SignalGroup = {
  id: string;
  label: string;
  rows: InsightRow[];
};

export type InsightModel = {
  copy: InsightsCopy;
  latestRun: AdviceRunView | null;
  latestFailure: AdviceRunView | null;
  adviceRows: InsightRow[];
  signalRows: InsightRow[];
  allRows: InsightRow[];
  actionRows: InsightRow[];
  openActionRows: InsightRow[];
  topRows: InsightRow[];
  signalGroups: SignalGroup[];
  maxSeverityCount: number;
  maxCategoryCount: number;
  adviceToolLabel: string;
  severityCount: (id: string) => number;
};

export const screenIds: MainInsightsScreen[] = ['overview', 'actions', 'signals', 'runs'];
export const actionFilterIds: ActionFilter[] = ['all', 'open', 'done', 'dismissed'];

const usd = new Intl.NumberFormat(undefined, {
  style: 'currency',
  currency: 'USD',
  maximumFractionDigits: 2
});

export function createInsightModel(snapshot: DesktopSnapshot): InsightModel {
  const copy = snapshot.copy.insights;
  const latestRun = snapshot.advice.runs[0] ?? null;
  const latestFailure = snapshot.advice.runs.find((run) => run.status === 'failed') ?? null;
  const adviceRows = buildAdviceRows(snapshot.advice.runs, copy, latestRun);
  const signalRows = snapshot.insights.recommendations.map((recommendation) => signalRow(recommendation, copy));
  const allRows = [...adviceRows, ...signalRows].sort(compareRows);
  const actionRows = [...adviceRows].sort(compareRows);
  const openActionRows = actionRows.filter((row) => row.status === 'open');
  const topRows = (openActionRows.length ? openActionRows : allRows).slice(0, 5);
  const maxSeverityCount = Math.max(0, ...snapshot.insights.summary.by_severity.map((severity) => severity.count));
  const maxCategoryCount = Math.max(0, ...snapshot.insights.summary.by_category.map((category) => category.count));
  const adviceToolLabel =
    snapshot.advice_tool_options.find((tool) => tool.value === snapshot.advice_tool)?.label ??
    snapshot.advice_tool;

  return {
    copy,
    latestRun,
    latestFailure,
    adviceRows,
    signalRows,
    allRows,
    actionRows,
    openActionRows,
    topRows,
    signalGroups: buildSignalGroups(signalRows),
    maxSeverityCount,
    maxCategoryCount,
    adviceToolLabel,
    severityCount: (id) =>
      snapshot.insights.summary.by_severity.find((severity) => severity.id === id)?.count ?? 0
  };
}

export function copyTemplate(template: string | undefined, values: Record<string, string>) {
  let out = template ?? '';
  for (const [key, value] of Object.entries(values)) {
    out = out.split(`{${key}}`).join(value);
  }
  return out;
}

export function buildAdviceRows(runs: AdviceRunView[], copy: InsightsCopy, latestRun: AdviceRunView | null) {
  const rows: InsightRow[] = [];
  for (const run of runs) {
    for (const item of run.items) {
      rows.push(adviceRow(run, item, copy, latestRun));
    }
  }
  return rows;
}

export function adviceRow(
  run: AdviceRunView,
  item: AdviceItemView,
  copy: InsightsCopy,
  latestRun: AdviceRunView | null
): InsightRow {
  const savings =
    item.estimated_savings_usd !== null
      ? copyTemplate(copy.estimated_savings, {
          amount: usd.format(item.estimated_savings_usd)
        })
      : null;

  return {
    id: `advice-${item.id}`,
    source: 'advice',
    sourceLabel: copy.source_advice,
    severity: normalizeSeverity(item.severity),
    severityLabel: severityLabel(item.severity, copy),
    category: item.category,
    categoryId: item.category,
    title: item.title,
    body: item.body,
    impact: item.impact,
    savings,
    savingsUsd: item.estimated_savings_usd,
    confidence: confidenceLabel(item, copy),
    status: item.status,
    statusLabel: adviceStatusLabel(item.status, copy),
    scopeLabel: runLabel(run, copy, latestRun),
    evidence: item.evidence,
    nextStep: item.next_step,
    assumption: item.notes,
    silencedReason: null,
    ruleId: null,
    run,
    advice: item,
    recommendation: null
  };
}

export function signalRow(recommendation: RecommendationView, copy: InsightsCopy): InsightRow {
  return {
    id: `signal-${recommendation.id}`,
    source: 'signal',
    sourceLabel: copy.source_signal,
    severity: recommendation.severity,
    severityLabel: recommendation.severity_label,
    category: recommendation.category_label,
    categoryId: recommendation.category,
    title: recommendation.title,
    body: recommendation.body,
    impact: recommendation.savings,
    savings: recommendation.savings,
    savingsUsd: recommendation.savings_amount_usd,
    confidence: null,
    status: 'signal',
    statusLabel: copy.status_signal,
    scopeLabel: recommendation.scope.label ?? copy.scope_all,
    evidence: recommendation.rule_id ? [recommendation.rule_id] : [],
    nextStep: null,
    assumption: recommendation.assumption,
    silencedReason: recommendation.silenced_reason,
    ruleId: recommendation.rule_id,
    run: null,
    advice: null,
    recommendation
  };
}

export function buildSignalGroups(rows: InsightRow[]): SignalGroup[] {
  const groups = new Map<string, SignalGroup>();
  for (const row of rows) {
    const existing = groups.get(row.categoryId);
    if (existing) {
      existing.rows.push(row);
    } else {
      groups.set(row.categoryId, {
        id: row.categoryId,
        label: row.category,
        rows: [row]
      });
    }
  }
  return [...groups.values()].sort((a, b) => b.rows.length - a.rows.length || a.label.localeCompare(b.label));
}

export function normalizeSeverity(severity: string): SeverityId {
  if (severity === 'risk' || severity === 'warn' || severity === 'info') return severity;
  return 'info';
}

export function severityLabel(severity: string, copy: InsightsCopy) {
  return copy.severity[severity] ?? severity;
}

export function severityRank(severity: SeverityId) {
  if (severity === 'risk') return 0;
  if (severity === 'warn') return 1;
  return 2;
}

export function compareRows(a: InsightRow, b: InsightRow) {
  return (
    rowStatusRank(a) - rowStatusRank(b) ||
    severityRank(a.severity) - severityRank(b.severity) ||
    (b.savingsUsd ?? -1) - (a.savingsUsd ?? -1) ||
    a.title.localeCompare(b.title)
  );
}

export function rowStatusRank(row: InsightRow) {
  if (row.source === 'advice' && row.status === 'open') return 0;
  if (row.source === 'signal') return 1;
  if (row.status === 'done') return 2;
  return 3;
}

export function rowMatchesActionFilter(row: InsightRow, filter: ActionFilter) {
  if (filter === 'all') return true;
  return row.status === filter;
}

export function screenLabel(screen: MainInsightsScreen, copy: InsightsCopy) {
  if (screen === 'actions') return copy.screen_actions;
  if (screen === 'signals') return copy.screen_signals;
  if (screen === 'runs') return copy.screen_runs;
  return copy.screen_overview;
}

export function actionFilterLabel(filter: ActionFilter, copy: InsightsCopy) {
  if (filter === 'open') return copy.filter_open;
  if (filter === 'done') return copy.filter_done;
  if (filter === 'dismissed') return copy.filter_dismissed;
  return copy.filter_all;
}

export function confidenceLabel(item: AdviceItemView, copy: InsightsCopy) {
  return copyTemplate(copy.advice_confidence, {
    confidence: `${Math.round(item.confidence * 100)}%`
  });
}

export function runStatusLabel(status: AdviceRunView['status'], copy: InsightsCopy) {
  return status === 'succeeded'
    ? copy.status_succeeded
    : copy.status_failed;
}

export function adviceStatusLabel(status: AdviceItemStatusId, copy: InsightsCopy) {
  if (status === 'done') return copy.status_done;
  if (status === 'dismissed') return copy.status_dismissed;
  return copy.status_open;
}

export function runScopeLabel(scope: AdviceDataScopeId, copy: InsightsCopy) {
  return scope === 'prompt_snippets'
    ? copy.advice_scope_snippets
    : copy.advice_scope_redacted;
}

export function runLabel(run: AdviceRunView, copy: InsightsCopy, latestRun: AdviceRunView | null) {
  const template =
    run.id === latestRun?.id
      ? copy.advice_latest
      : copy.advice_run;
  return copyTemplate(template, {
    tool: run.tool_label,
    scope: runScopeLabel(run.data_scope, copy),
    status: runStatusLabel(run.status, copy)
  });
}

export function runDetail(run: AdviceRunView, copy: InsightsCopy) {
  if (run.status === 'failed') {
    return copyTemplate(copy.advice_failed, {
      error: run.error ?? runStatusLabel(run.status, copy)
    });
  }
  if (run.summary) return run.summary;
  return copyTemplate(copy.run_items, {
    count: String(run.items.length)
  });
}

export function generatedAtLabel(value: string, copy: InsightsCopy) {
  const parsed = new Date(value);
  const label = Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
  return copyTemplate(copy.generated_at_value, { value: label });
}

export function baselineLabel(days: number, copy: InsightsCopy) {
  return copyTemplate(copy.baseline_window_value, {
    days: String(days)
  });
}

export function archiveSourceLabel(snapshot: DesktopSnapshot) {
  return snapshot.source === 'live'
    ? snapshot.copy.status.live_data
    : snapshot.copy.status.sample_data;
}

export function formatRunDate(value: string) {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

export function barStyle(count: number, max: number) {
  const width = max <= 0 || count <= 0 ? 0 : Math.max(4, Math.round((count / max) * 100));
  return `--bar-width: ${width}%;`;
}
