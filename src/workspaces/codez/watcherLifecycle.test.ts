import { describe, expect, it, vi } from "vitest";
import { startOwnedWatcher } from "./watcherLifecycle";

describe("startOwnedWatcher", () => {
  it("waits for an in-flight start before stopping after unmount", async () => {
    let finishStart!: () => void;
    const start = vi.fn(() => new Promise<void>((resolve) => { finishStart = resolve; }));
    const stop = vi.fn(async () => {});
    const dispose = startOwnedWatcher(start, stop);
    await Promise.resolve();
    dispose();
    dispose();
    expect(stop).not.toHaveBeenCalled();
    finishStart();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(stop).toHaveBeenCalledOnce();
  });

  it("does not stop a watcher that failed to start", async () => {
    const stop = vi.fn(async () => {});
    const dispose = startOwnedWatcher(async () => { throw new Error("unavailable"); }, stop);
    dispose();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(stop).not.toHaveBeenCalled();
  });
});
