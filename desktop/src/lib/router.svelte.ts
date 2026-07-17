import type { ToolId } from '../types';

export type RoutePage =
  | 'overview'
  | 'analytics'
  | 'coach'
  | 'tools'
  | 'models'
  | 'projects'
  | 'config'
  | 'session';

export type RouteToolId = Exclude<ToolId, 'all'>;

export type Route = {
  page: RoutePage;
  tool?: RouteToolId;
  sessionKey?: string;
  /** Projects: set for the per-project sub-route, absent on the index. */
  project?: { identity: string; label: string };
};

/** Pages reachable from the sidebar, in nav (and Tab-cycle) order. */
export const NAV_PAGES: RoutePage[] = ['overview', 'analytics', 'coach', 'models', 'projects', 'tools', 'config'];

let route = $state<Route>({ page: 'overview' });

export const router = {
  get route(): Route {
    return route;
  },
  go(next: Route) {
    route = next;
  },
  cycle(delta: number) {
    const pages = NAV_PAGES;
    const idx = pages.indexOf(route.page);
    const base = idx === -1 ? 0 : idx;
    const next = pages[(base + delta + pages.length) % pages.length];
    route = { page: next };
  }
};
