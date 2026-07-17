export function count(value: number) {
  return value.toLocaleString();
}

export function rankPercent(value: number) {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(100, Math.round(value)));
}

export function rankLabel(template: string, value: number) {
  return template.replace('{value}', String(rankPercent(value)));
}
