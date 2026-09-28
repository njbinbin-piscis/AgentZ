import { afterEach, describe, expect, it } from "vitest";
import { extensionUiStore as store, MAX_OUTPUT_CHARS } from "./extensionUiStore";

afterEach(() => store.reset());

describe("extension output retention", () => {
  it("bounds continuous output and retains its newest tail", () => {
    store.registerOutput("test", "test");
    for (let i = 0; i < 100; i++) store.appendOutput("test", "x".repeat(16_384));
    store.appendOutput("test", "last message");
    const output = store.getSnapshot().outputChannels[0].content;
    expect(output.length).toBe(MAX_OUTPUT_CHARS);
    expect(output.endsWith("last message")).toBe(true);
  });

  it("bounds individual host/debug messages as well as message counts", () => {
    const huge = "x".repeat(100_000);
    store.appendHostLog(huge);
    store.appendDebugOutput(huge);
    expect(store.getSnapshot().hostLog[store.getSnapshot().hostLog.length - 1]?.length).toBeLessThanOrEqual(4096);
    expect(store.getSnapshot().debugOutput[store.getSnapshot().debugOutput.length - 1]?.length).toBeLessThanOrEqual(4096);
  });
});
