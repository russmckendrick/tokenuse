/** Chart colors route through design tokens — never hex literals (DESIGN.md). */

const SERIES = [
  'var(--chart-series-1)',
  'var(--chart-series-2)',
  'var(--chart-series-3)',
  'var(--chart-series-4)',
  'var(--chart-series-5)',
  'var(--chart-series-6)',
  'var(--chart-series-7)',
  'var(--chart-series-8)'
];

/** Stable tool accents shared with the Usage console tones. */
const TOOL_COLORS: Record<string, string> = {
  codex: 'var(--color-primary)',
  'claude-code': 'var(--color-magenta)',
  cursor: 'var(--color-secondary)',
  copilot: 'var(--color-tertiary)',
  gemini: 'var(--color-cyan)'
};

const PROVIDER_COLORS: Record<string, string> = {
  anthropic: 'var(--provider-anthropic)',
  openai: 'var(--provider-openai)',
  google: 'var(--provider-google)',
  github: 'var(--provider-github)',
  cursor: 'var(--provider-cursor)',
  xai: 'var(--provider-xai)'
};

export function seriesColor(index: number): string {
  return SERIES[index % SERIES.length];
}

export function toolColor(toolId: string, fallbackIndex = 0): string {
  return TOOL_COLORS[toolId] ?? seriesColor(fallbackIndex);
}

export function providerColor(providerId: string): string {
  return PROVIDER_COLORS[providerId] ?? 'var(--provider-other)';
}
