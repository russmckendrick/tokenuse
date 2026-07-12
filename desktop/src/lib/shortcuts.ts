import type { PeriodId } from '../types';

/**
 * Client-side keyboard vocabulary for the desktop shell. Navigation and
 * modal keys resolve here; data actions call typed commands directly. The
 * hint copy in `copy.keymap.footers` mirrors these bindings.
 */

export const PERIOD_KEYS: Record<string, PeriodId> = {
  '1': 'today',
  '2': 'week',
  '3': 'thirty-days',
  '4': 'month',
  '5': 'all-time'
};

export type ShortcutAction =
  | { kind: 'period'; period: PeriodId }
  | { kind: 'cycle-tool' }
  | { kind: 'cycle-sort' }
  | { kind: 'toggle-source' }
  | { kind: 'open-project-picker' }
  | { kind: 'open-session-picker' }
  | { kind: 'open-report' }
  | { kind: 'refresh' }
  | { kind: 'escape' };

export function resolveShortcut(event: KeyboardEvent): ShortcutAction | null {
  if (event.key === 'Escape') return { kind: 'escape' };
  if (event.ctrlKey || event.altKey || event.metaKey) return null;

  if (event.shiftKey) {
    return event.key === 'D' ? { kind: 'toggle-source' } : null;
  }

  const period = PERIOD_KEYS[event.key];
  if (period) return { kind: 'period', period };

  switch (event.key) {
    case 't':
      return { kind: 'cycle-tool' };
    case 'g':
      return { kind: 'cycle-sort' };
    case 'p':
      return { kind: 'open-project-picker' };
    case 's':
      return { kind: 'open-session-picker' };
    case 'e':
      return { kind: 'open-report' };
    case 'r':
      return { kind: 'refresh' };
    default:
      return null;
  }
}
