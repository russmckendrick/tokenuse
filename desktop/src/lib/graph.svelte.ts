import type { GraphData, GraphMetricId } from '../types';

export type GraphLens = 'projects' | 'stack';

export type GraphCamera = {
  x: number;
  y: number;
  z: number;
  targetX: number;
  targetY: number;
  targetZ: number;
};

export type GraphPageState = {
  lens: GraphLens;
  metric: GraphMetricId;
  showCoreTools: boolean;
  showMcpServers: boolean;
  selectedId: string | null;
  camera: GraphCamera | null;
  data: GraphData | null;
  dataKey: string;
  error: boolean;
  loading: boolean;
};

const state = $state<GraphPageState>({
  lens: 'projects',
  metric: 'calls',
  showCoreTools: false,
  showMcpServers: false,
  selectedId: null,
  camera: null,
  data: null,
  dataKey: '',
  error: false,
  loading: false
});

let requestSeq = 0;

/**
 * Session-scoped graph explorer state. Keeping it outside the route component
 * preserves the lens, selection, and camera while a project/model drill-in is
 * open, without persisting local project names beyond the process.
 */
export const graphView = {
  get state(): GraphPageState {
    return state;
  },
  nextRequest(): number {
    requestSeq += 1;
    return requestSeq;
  },
  isCurrent(seq: number): boolean {
    return seq === requestSeq;
  }
};
