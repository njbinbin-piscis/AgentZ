import { describe, expect, it } from "vitest";
import { acquireTerminal, terminalBackendId } from "./terminalLifecycle";
const deferred = () => { let resolve!: () => void; const promise = new Promise<void>(r => { resolve = r; }); return {promise, resolve}; };
describe("terminal lifecycle ordering", () => {
  it("late creation is destroyed before a replacement can create", async () => {
    const gate = deferred(); const started = deferred(); const events: string[] = [];
    const first = acquireTerminal("late", async () => { events.push("old-create"); started.resolve(); await gate.promise; }, async () => { events.push("old-destroy"); });
    await started.promise;
    const cleanup = first.dispose();
    const second = acquireTerminal("late", async () => { events.push("new-create"); }, async () => { events.push("new-destroy"); });
    gate.resolve(); await Promise.all([first.ready, cleanup, second.ready]);
    expect(events).toEqual(["old-create", "old-destroy", "new-create"]);
    await first.dispose(); expect(events).toHaveLength(3);
    await second.dispose(); expect(events[events.length - 1]).toBe("new-destroy");
  });
  it("failed cleanup blocks replacement creation", async () => {
    const error = new Error("cleanup failed"); let replacement = false;
    const first = acquireTerminal("failed-cleanup", async () => {}, async () => { throw error; });
    await first.ready;
    await expect(first.dispose()).rejects.toThrow("cleanup failed");
    const second = acquireTerminal("failed-cleanup", async () => { replacement = true; }, async () => {});
    await expect(second.ready).rejects.toThrow("cleanup failed");
    await expect(second.dispose()).rejects.toThrow("cleanup failed");
    expect(replacement).toBe(false);
  });
  it("a failed create cannot silently permit a replacement", async () => {
    const first = acquireTerminal("failed-create", async () => { throw new Error("create uncertain"); }, async () => {});
    await expect(first.ready).rejects.toThrow("create uncertain");
    await expect(first.dispose()).rejects.toThrow("create uncertain");
    let created = false;
    const second = acquireTerminal("failed-create", async () => { created = true; }, async () => {});
    await expect(second.ready).rejects.toThrow("create uncertain");
    expect(created).toBe(false);
    await expect(second.dispose()).rejects.toThrow("create uncertain");
  });
  it("a stalled terminal does not block another ID", async () => {
    const gate = deferred();
    const first = acquireTerminal("stalled", () => gate.promise, async () => {});
    let created = false;
    const other = acquireTerminal("independent", async () => { created = true; }, async () => {});
    await other.ready; expect(created).toBe(true); await other.dispose();
    gate.resolve(); await first.ready; await first.dispose();
  });
  it("cancellation before dispatch never creates", async () => {
    let calls = 0;
    const lease = acquireTerminal("cancel", async () => { calls++; }, async () => { calls++; });
    await lease.dispose(); await lease.ready; expect(calls).toBe(0);
  });
});

it("successful cleanup removes only its owned backend lookup", async () => {
  const lease = acquireTerminal("lookup-cleanup", async () => {}, async () => {}, "physical-lookup");
  await lease.ready; expect(terminalBackendId("lookup-cleanup")).toBe("physical-lookup");
  await lease.dispose(); expect(terminalBackendId("lookup-cleanup")).toBeUndefined();
});
