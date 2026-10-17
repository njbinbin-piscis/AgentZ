/**
 * First-run guidance for the codez assistant. Pure helpers so the rules can be
 * unit-tested without rendering; the UI lives in FirstRunChecklist.tsx.
 */

/** localStorage key recording that a first task has completed successfully. */
export const FIRST_TASK_DONE_KEY = "agentz.firstTaskDone.v1";

/** Window event that asks the shell to open the Settings panel. */
export const OPEN_SETTINGS_EVENT = "agentz:open-settings";
/** Window event dispatched when the Settings panel closes, so setup state can be re-read. */
export const SETTINGS_CLOSED_EVENT = "agentz:settings-closed";

export type FirstRunStepId = "project" | "api_key" | "first_task";
export type FirstRunStepStatus = "done" | "todo" | "unknown";

export interface FirstRunStep {
  id: FirstRunStepId;
  status: FirstRunStepStatus;
}

interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function defaultStore(): KeyValueStore | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function hasFirstTaskDone(store: KeyValueStore | null = defaultStore()): boolean {
  try {
    return store?.getItem(FIRST_TASK_DONE_KEY) === "1";
  } catch {
    return false;
  }
}

export function markFirstTaskDone(store: KeyValueStore | null = defaultStore()): void {
  try {
    store?.setItem(FIRST_TASK_DONE_KEY, "1");
  } catch {
    // Storage unavailable (private mode, quota): the checklist simply keeps showing.
  }
}

/** Checklist state. `configured` is null while the settings check is still pending. */
export function firstRunSteps(input: { projectReady: boolean; configured: boolean | null; firstTaskDone: boolean }): FirstRunStep[] {
  const keyStatus: FirstRunStepStatus =
    input.configured === null ? "unknown" : input.configured ? "done" : "todo";
  return [
    { id: "project", status: input.projectReady ? "done" : "todo" },
    { id: "api_key", status: keyStatus },
    { id: "first_task", status: input.firstTaskDone ? "done" : "todo" },
  ];
}

const SETUP_FAILURE_PATTERNS: RegExp[] = [
  /\b401\b/,
  /\b402\b/,
  /\b403\b/,
  /unauthori[sz]ed/,
  /invalid[_ -]?api[_ -]?key/,
  /api[_ -]?key/,
  /missing .*key/,
  /not configured/,
  /insufficient[_ ]balance/,
  /未配置/,
  /密钥/,
];

/**
 * True when a chat error points at model setup (missing or rejected credentials,
 * or an unusable account). Those errors map to the Settings entry point.
 */
export function isLlmSetupFailure(message: string): boolean {
  const lower = message.toLowerCase();
  return SETUP_FAILURE_PATTERNS.some((pattern) => pattern.test(lower));
}

/** A next step for first-task failures that are outside model credential setup. */
export function firstTaskFailureHint(message: string): "project" | "tool" | "connection" | null {
  const lower = message.toLowerCase();
  if (/directory not found|workspace.*(missing|not found)|project.*(missing|not found)|目录不存在|工作区不存在/.test(lower)) return "project";
  if (/tool .* (unavailable|not found|disabled)|unknown tool|工具.*(不可用|未找到)/.test(lower)) return "tool";
  if (/connection refused|network timeout|econnrefused|enotfound|连接失败|网络超时/.test(lower)) return "connection";
  return null;
}

/** Ask the shell to open Settings. App.tsx listens for `OPEN_SETTINGS_EVENT`. */
export function requestOpenSettings(): void {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT));
}
