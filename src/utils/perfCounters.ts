/** Dev-only performance counters for diagnosing refresh / watcher storms. */

interface PerfSnapshot {
  watcherEventsReceived: number;
  watcherEventsIgnored: number;
  workspaceRefreshScheduled: number;
  workspaceRefreshFlushed: number;
  tabReloadScheduled: number;
}

const counters: PerfSnapshot = {
  watcherEventsReceived: 0,
  watcherEventsIgnored: 0,
  workspaceRefreshScheduled: 0,
  workspaceRefreshFlushed: 0,
  tabReloadScheduled: 0,
};

function enabled(): boolean {
  return import.meta.env.DEV;
}

/** Long-lived resources whose open/close balance must return to zero after a session cycle. */
export type ResourceKind = "listener" | "terminal" | "editorModel" | "extHostSession" | "ptySession";

export interface ResourceCount {
  opened: number;
  closed: number;
  /** opened - closed; a steady non-zero value after a cycle is a leak. */
  live: number;
  /** Close calls with no matching open (double dispose or missed open). */
  unmatchedClose: number;
}

const RESOURCE_KINDS: ResourceKind[] = ["listener", "terminal", "editorModel", "extHostSession", "ptySession"];
const resources: Record<ResourceKind, ResourceCount> = Object.fromEntries(
  RESOURCE_KINDS.map((kind) => [kind, { opened: 0, closed: 0, live: 0, unmatchedClose: 0 }]),
) as Record<ResourceKind, ResourceCount>;

export const perfCounters = {
  trackListener(unlisten: () => void): () => void {
    perfCounters.resourceOpened("listener");
    let closed = false;
    return () => {
      if (closed) return;
      unlisten();
      closed = true;
      perfCounters.resourceClosed("listener");
    };
  },
  resourceOpened(kind: ResourceKind): void {
    if (!enabled()) return;
    const r = resources[kind];
    r.opened += 1;
    r.live += 1;
  },
  resourceClosed(kind: ResourceKind): void {
    if (!enabled()) return;
    const r = resources[kind];
    if (r.live === 0) {
      r.unmatchedClose += 1;
      return;
    }
    r.closed += 1;
    r.live -= 1;
  },
  resourceSnapshot(): Record<ResourceKind, ResourceCount> {
    return Object.fromEntries(
      RESOURCE_KINDS.map((kind) => [kind, { ...resources[kind] }]),
    ) as Record<ResourceKind, ResourceCount>;
  },
  resetResources(): void {
    for (const kind of RESOURCE_KINDS) resources[kind] = { opened: 0, closed: 0, live: 0, unmatchedClose: 0 };
  },
  recordWatcherReceived(): void {
    if (enabled()) counters.watcherEventsReceived += 1;
  },
  recordWatcherIgnored(): void {
    if (enabled()) counters.watcherEventsIgnored += 1;
  },
  recordWorkspaceRefreshScheduled(): void {
    if (enabled()) counters.workspaceRefreshScheduled += 1;
  },
  recordWorkspaceRefreshFlushed(): void {
    if (enabled()) counters.workspaceRefreshFlushed += 1;
  },
  recordTabReloadScheduled(): void {
    if (enabled()) counters.tabReloadScheduled += 1;
  },
  snapshot(): PerfSnapshot {
    return { ...counters };
  },
  reset(): void {
    counters.watcherEventsReceived = 0;
    counters.watcherEventsIgnored = 0;
    counters.workspaceRefreshScheduled = 0;
    counters.workspaceRefreshFlushed = 0;
    counters.tabReloadScheduled = 0;
  },
};

if (import.meta.env.DEV && typeof window !== "undefined") {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__agentz_perf = perfCounters;
}
