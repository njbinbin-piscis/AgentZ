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
