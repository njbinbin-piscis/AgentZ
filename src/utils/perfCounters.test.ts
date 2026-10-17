import { describe, expect, it, beforeEach } from "vitest";
import { perfCounters, type ResourceKind } from "./perfCounters";

const KINDS: ResourceKind[] = ["listener", "terminal", "editorModel", "extHostSession", "ptySession"];

/** One open/close session cycle: the same resources a project switch or editor toggle creates. */
function openAndCloseSession(): void {
  for (const kind of KINDS) perfCounters.resourceOpened(kind);
  for (const kind of KINDS) perfCounters.resourceClosed(kind);
}

describe("perfCounters resource balance", () => {
  beforeEach(() => perfCounters.resetResources());

  it("returns every live count to zero after 50 open/close cycles", () => {
    for (let i = 0; i < 50; i += 1) openAndCloseSession();
    const snap = perfCounters.resourceSnapshot();
    for (const kind of KINDS) {
      expect(snap[kind], kind).toEqual({ opened: 50, closed: 50, live: 0, unmatchedClose: 0 });
    }
  });

  it("reports a missed close as a live resource", () => {
    perfCounters.resourceOpened("listener");
    perfCounters.resourceOpened("listener");
    perfCounters.resourceClosed("listener");
    expect(perfCounters.resourceSnapshot().listener.live).toBe(1);
  });

  it("counts a close without a matching open instead of going negative", () => {
    perfCounters.resourceClosed("terminal");
    perfCounters.resourceClosed("terminal");
    const terminal = perfCounters.resourceSnapshot().terminal;
    expect(terminal.live).toBe(0);
    expect(terminal.unmatchedClose).toBe(2);
  });

  it("keeps the legacy watcher counters working alongside resource counts", () => {
    perfCounters.reset();
    perfCounters.recordWatcherReceived();
    expect(perfCounters.snapshot().watcherEventsReceived).toBe(1);
  });
});
