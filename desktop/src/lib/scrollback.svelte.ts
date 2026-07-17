import type { ScrollbackResults } from '../types';

/**
 * Scrollback page state, kept outside the component (router.svelte.ts
 * pattern) so drilling into a session and returning restores the query,
 * filters, results, and scroll position without refetching.
 */
export type ScrollbackPageState = {
  query: string;
  /** Tool id filter; empty string = all tools. */
  tool: string;
  /** Project identity filter; empty string = all projects. */
  project: string;
  results: ScrollbackResults | null;
  error: string | null;
  searching: boolean;
};

const state = $state<ScrollbackPageState>({
  query: '',
  tool: '',
  project: '',
  results: null,
  error: null,
  searching: false
});

/**
 * Monotonic request counter, module-level like the state itself: a component
 * instance can unmount (session drill-down) while its search is in flight,
 * and a fresh instance's searches must still outrank the stale response.
 */
let requestSeq = 0;

export const scrollback = {
  get state(): ScrollbackPageState {
    return state;
  },
  /** Claim a new request id, invalidating every in-flight response. */
  nextRequest(): number {
    requestSeq += 1;
    return requestSeq;
  },
  isCurrent(seq: number): boolean {
    return seq === requestSeq;
  }
};
