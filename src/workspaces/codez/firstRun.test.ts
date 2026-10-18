import { describe, expect, it } from "vitest";
import {
  FIRST_TASK_DONE_KEY,
  firstTaskFailureHint,
  hasFirstTaskDone,
  isLlmSetupFailure,
  markFirstTaskDone,
} from "./firstRun";

function memoryStore() {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
  };
}

describe("firstTaskFailureHint", () => {
  it.each([
    ["Directory not found: C:\\missing", "project"],
    ["unknown tool: shell_execute", "tool"],
    ["connection refused by endpoint", "connection"],
    ["rate limited, retry later", null],
  ] as const)("classifies %s", (message, expected) => {
    expect(firstTaskFailureHint(message)).toBe(expected);
  });
});

describe("first-task persistence", () => {
  it("is not done until marked, then stays done", () => {
    const store = memoryStore();
    expect(hasFirstTaskDone(store)).toBe(false);
    markFirstTaskDone(store);
    expect(store.data.get(FIRST_TASK_DONE_KEY)).toBe("1");
    expect(hasFirstTaskDone(store)).toBe(true);
  });

  it("degrades to not-done when storage is unavailable", () => {
    expect(hasFirstTaskDone(null)).toBe(false);
    expect(() => markFirstTaskDone(null)).not.toThrow();
  });
});

describe("isLlmSetupFailure", () => {
  it.each([
    "HTTP 401 Unauthorized",
    "invalid_api_key: incorrect key provided",
    "API key is missing for provider openai",
    "Provider anthropic is not configured",
    "请检查 API 密钥",
    "402 insufficient_balance",
  ])("maps %s to the Settings entry point", (message) => {
    expect(isLlmSetupFailure(message)).toBe(true);
  });

  it.each(["network timeout after 30s", "tool read_file failed: no such file", "rate limited, retry later"])(
    "does not treat %s as a setup failure",
    (message) => {
      expect(isLlmSetupFailure(message)).toBe(false);
    },
  );
});
