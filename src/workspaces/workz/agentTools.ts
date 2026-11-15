import { settleSubagentActivity } from "../../components/subagentCard";
import { settledToolStatus } from "../../components/toolCancel";
import type { AgentEvent } from "../../services/tauri/chat";
import { pathFromToolEvent, type AgentToolEvent } from "./agentArtifacts";

export function applyToolStart(tools: AgentToolEvent[], evt: Extract<AgentEvent, { type: "tool_start" }>, textOffset?: number): AgentToolEvent[] {
  const idx = tools.findIndex((t) => t.id === evt.id);
  const next = {
    id: evt.id,
    name: evt.name,
    status: "running" as const,
    input: evt.input,
    path: pathFromToolEvent(evt.name, evt.input),
  };
  if (idx >= 0) {
    return tools.map((t, i) => (i === idx ? { ...t, ...next } : t));
  }
  return [...tools, { ...next, textOffset }];
}

export function applyToolEnd(tools: AgentToolEvent[], evt: Extract<AgentEvent, { type: "tool_end" }>): AgentToolEvent[] {
  // A call cut short by Stop reports an error result, but it is not a failure.
  const settled = settledToolStatus(evt.result, evt.is_error);
  const path = pathFromToolEvent(evt.name, undefined, evt.result);

  const patch = (t: AgentToolEvent): AgentToolEvent => ({
    ...t,
    id: evt.id,
    name: evt.name,
    ...settled,
    result: evt.result,
    path: path ?? t.path,
    activity: settleSubagentActivity(t.activity, evt.is_error),
  });

  const byId = tools.findIndex((t) => t.id === evt.id);
  if (byId >= 0) {
    return tools.map((t, i) => (i === byId ? patch(t) : t));
  }

  // Race: tool_end may arrive before tool_start state lands — match last in-flight call.
  for (let i = tools.length - 1; i >= 0; i--) {
    if (tools[i].name === evt.name && tools[i].status === "running") {
      return tools.map((t, idx) => (idx === i ? patch(t) : t));
    }
  }

  return [
    ...tools,
    {
      id: evt.id,
      name: evt.name,
      ...settled,
      result: evt.result,
      path,
    },
  ];
}

/** Clear stale in-flight markers when the turn finishes. */
export function finalizeTools(tools: AgentToolEvent[]): AgentToolEvent[] {
  return tools.map((t) => (t.status === "running" ? { ...t, status: "done" as const } : t));
}

/** A stop the backend never confirmed: calls still in flight did not complete. */
export function cancelRunningTools(tools: AgentToolEvent[]): AgentToolEvent[] {
  return tools.map((t) => (t.status === "running" ? { ...t, status: "cancelled" as const } : t));
}
