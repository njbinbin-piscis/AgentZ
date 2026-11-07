import type { ToolTraceItem } from "./ToolTrace";

const SUBAGENT_TOOLS = new Set(["task", "delegate", "call_fish"]);

export interface SubagentView {
  agent: string;
  brief: string;
  status: ToolTraceItem["status"];
  toolCalls: number | null;
  tokens: number | null;
  cached: boolean;
  fallback: boolean;
  report: string;
}

/** One visible step of a running or finished sub-agent. */
export interface SubagentStep {
  id: string;
  iteration: number;
  phase: "thinking" | "tool";
  status: "running" | "done" | "error";
  name: string;
  detail: string;
}

export interface SubagentProgress {
  type: "subagent_progress";
  tool_id: string;
  step_id: string;
  iteration: number;
  phase: "thinking" | "tool";
  status: "running" | "done" | "error";
  name: string;
  detail: string;
}

const MAX_SUBAGENT_STEPS = 80;

/** Attach or update one live step on the parent tool card it belongs to. */
export function applySubagentProgress<T extends { id: string; activity?: SubagentStep[] }>(
  items: T[],
  evt: SubagentProgress,
): T[] {
  const idx = items.findIndex((item) => item.id === evt.tool_id);
  if (idx < 0) return items;
  const activity = items[idx].activity ? items[idx].activity!.slice() : [];
  const step: SubagentStep = {
    id: evt.step_id,
    iteration: evt.iteration,
    phase: evt.phase,
    status: evt.status,
    name: evt.name,
    detail: evt.detail,
  };
  const at = activity.findIndex((s) => s.id === step.id);
  if (at >= 0) {
    activity[at] = {
      ...step,
      name: step.name || activity[at].name,
      detail: step.detail || activity[at].detail,
    };
  } else {
    activity.push(step);
  }
  const capped = activity.length > MAX_SUBAGENT_STEPS ? activity.slice(activity.length - MAX_SUBAGENT_STEPS) : activity;
  const next = items.slice();
  next[idx] = { ...items[idx], activity: capped };
  return next;
}

/** A finished parent call should not leave its last child step spinning. */
export function settleSubagentActivity(
  steps: SubagentStep[] | undefined,
  failed: boolean,
): SubagentStep[] | undefined {
  if (!steps) return steps;
  let lastRunning = -1;
  steps.forEach((step, i) => {
    if (step.status === "running") lastRunning = i;
  });
  return steps.map((step, i) => {
    if (step.status !== "running") return step;
    if (failed && i === lastRunning) return { ...step, status: "error" };
    return { ...step, status: "done" };
  });
}

export function indexSubagentActivity(
  tools: Array<{ id: string; activity?: SubagentStep[] }>,
): Map<string, SubagentStep[]> {
  const map = new Map<string, SubagentStep[]>();
  for (const tool of tools) {
    if (tool.activity && tool.activity.length > 0) map.set(tool.id, tool.activity);
  }
  return map;
}

export function subagentStepText(
  step: SubagentStep,
  t: (key: string, opts?: Record<string, unknown>) => string,
): string {
  if (step.phase === "thinking") return t("chat.subagentStepThinking", { n: step.iteration });
  const detail = step.detail.trim();
  return detail ? `${step.name} · ${detail}` : step.name || t("chat.subagentStepTool");
}

function asRecord(input: unknown): Record<string, unknown> {
  if (input && typeof input === "object") return input as Record<string, unknown>;
  if (typeof input === "string") {
    try {
      const parsed: unknown = JSON.parse(input);
      if (parsed && typeof parsed === "object") return parsed as Record<string, unknown>;
    } catch {
      /* streamed partial JSON: fall through */
    }
  }
  return {};
}

/** True for delegation calls that run a sub-agent (not `task` with action=list). */
export function isSubagentCall(item: Pick<ToolTraceItem, "name" | "input">): boolean {
  if (!SUBAGENT_TOOLS.has(item.name)) return false;
  const action = asRecord(item.input).action;
  return action !== "list";
}

const HEADER = /^(.*?) report \(([^)]*)\):\n\n/s;

export function parseSubagent(
  item: Pick<ToolTraceItem, "name" | "input" | "result" | "status">,
): SubagentView {
  const input = asRecord(item.input);
  const str = (v: unknown) => (typeof v === "string" ? v : "");
  let agent = "";
  let brief = "";
  if (item.name === "task") {
    agent = str(input.agent_id);
    brief = str(input.brief);
  } else if (item.name === "call_fish") {
    agent = str(input.fish);
    brief = str(input.task);
  } else {
    agent = "research";
    brief = str(input.task);
  }

  const result = item.result ?? "";
  let toolCalls: number | null = null;
  let tokens: number | null = null;
  let cached = false;
  let fallback = false;
  let report = result;

  const m = HEADER.exec(result);
  if (m) {
    const meta = m[2];
    cached = meta.startsWith("cached");
    fallback = meta.includes("main-model fallback");
    const calls = /(\d+) tool calls/.exec(meta);
    if (calls) toolCalls = Number(calls[1]);
    const tok = /(\d+)\+(\d+) tokens/.exec(meta);
    if (tok) tokens = Number(tok[1]) + Number(tok[2]);
    report = result.slice(m[0].length);
  } else {
    report = result.replace(/^Sub-agent findings:\n\n/, "").replace(/^[^\n]* report:\n\n/, "");
  }
  return { agent, brief, status: item.status, toolCalls, tokens, cached, fallback, report };
}
