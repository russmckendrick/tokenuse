export type CoachTone = 'good' | 'warn' | 'bad';

// Coach grades on a curve (DESIGN.md "Gauges"): green is earned by A-tier
// only, B/C read as needs-attention, D/F as trouble. Mirrors
// GRADE_TONE_GOOD / GRADE_TONE_WARN in src/coach/score.rs so the radial
// gauge, practice score bars, sparklines, and grade badges always agree.
const GRADE_GOOD = 90;
const GRADE_WARN = 70;

// Flow labels come from src/coach/flow.rs (FLOW_DEEP / FLOW_MODERATE), so the
// badge tone tracks the label it sits beside.
const FLOW_GOOD = 70;
const FLOW_WARN = 45;

function tone(score: number, good: number, warn: number): CoachTone {
  const value = Number.isFinite(score) ? score : 0;
  return value >= good ? 'good' : value >= warn ? 'warn' : 'bad';
}

export function gradeTone(score: number): CoachTone {
  return tone(score, GRADE_GOOD, GRADE_WARN);
}

export function flowTone(score: number): CoachTone {
  return tone(score, FLOW_GOOD, FLOW_WARN);
}
