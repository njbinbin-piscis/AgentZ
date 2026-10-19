import type { DurableRunEvent, RunEventStream } from "./runEvents";

export interface PassiveRunProjection {
  text: string;
  tools: { id: string; name: string; status: "running" | "completed" | "failed" }[];
  outcome: "unknown" | "done" | "cancelled" | "error";
  errorCount: number;
  ignoredCount: number;
  truncated: boolean;
}
const MAX_EVENTS = 5000;
const MAX_TEXT = 65536;
const MAX_TOOLS = 100;
function own(value: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}
function object(value: unknown): Record<string, unknown> {
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  )
    throw new Error("Invalid agent event object");
  return value as Record<string, unknown>;
}
function fields(value: Record<string, unknown>, required: string[], optional: string[] = []): void {
  if (
    required.some((key) => !own(value, key)) ||
    Object.keys(value).some((key) => !required.includes(key) && !optional.includes(key))
  )
    throw new Error("Unknown or missing agent event fields");
}
function string(value: unknown): string {
  if (typeof value !== "string" || value.length > 1_048_576)
    throw new Error("Invalid agent event string");
  return value;
}
function count(value: unknown): void {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 4294967295)
    throw new Error("Invalid agent event count");
}
function json(value: unknown): void {
  let nodes = 0;
  function visit(item: unknown, depth: number): void {
    if (++nodes > 10000 || depth > 16) throw new Error("Agent payload shape budget exceeded");
    if (item === null || typeof item === "boolean") return;
    if (typeof item === "string") {
      string(item);
      return;
    }
    if (typeof item === "number" && Number.isFinite(item)) return;
    if (Array.isArray(item)) {
      item.forEach((child) => visit(child, depth + 1));
      return;
    }
    const record = object(item);
    Object.values(record).forEach((child) => visit(child, depth + 1));
  }
  visit(value, 0);
}
function redacted(text: string): string {
  // Plain text only. Raw tool inputs/results, error details and approval payloads are never projected.
  return text
    .replace(/\bBearer\s+[^\s"'<>]+/gi, "Bearer [redacted]")
    .replace(/\bsk-[A-Za-z0-9_-]{12,}/g, "[redacted]")
    .replace(
      /\b(api[_-]?key|access[_-]?token|password|secret)\s*[:=]\s*[^\s,;]+/gi,
      "$1=[redacted]",
    );
}
function streamKey(stream: RunEventStream): string {
  const record = object(stream);
  fields(record, ["task_id", "run_id", "attempt_id", "environment_id"]);
  const values = [stream.task_id, stream.run_id, stream.attempt_id, stream.environment_id];
  if (
    values.some(
      (value) =>
        typeof value !== "string" ||
        !value ||
        value.length > 256 ||
        value.trim() !== value ||
        Array.from(value).some(
          (character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
        ),
    )
  )
    throw new Error("Invalid projection stream");
  return JSON.stringify(values);
}

/** Passive display snapshot for one authoritative Attempt; no actions, permissions or model resumption. */
export function projectRunAgentEvents(
  stream: RunEventStream,
  events: readonly DurableRunEvent[],
): PassiveRunProjection {
  const key = streamKey(stream);
  if (!Array.isArray(events) || events.length > MAX_EVENTS)
    throw new Error("Agent projection event budget exceeded");
  const result: PassiveRunProjection = {
    text: "",
    tools: [],
    outcome: "unknown",
    errorCount: 0,
    ignoredCount: 0,
    truncated: false,
  };
  const tools = new Map<
    string,
    { id: string; name: string; status: "running" | "completed" | "failed" }
  >();
  let previous = 0;
  let rawText = "";
  for (const event of events) {
    if (
      streamKey(event.cursor.stream) !== key ||
      !Number.isSafeInteger(event.cursor.sequence) ||
      event.cursor.sequence < 1 ||
      (previous && event.cursor.sequence !== previous + 1)
    )
      throw new Error("Invalid projection event scope/order");
    previous = event.cursor.sequence;
    if (event.kind !== "agent.event") {
      result.ignoredCount += 1;
      continue;
    }
    const payload = object(event.payload);
    const type = string(payload.type);
    switch (type) {
      case "text_delta": {
        fields(payload, ["type", "delta"]);
        const delta = string(payload.delta);
        const capacity = MAX_TEXT - rawText.length;
        if (delta.length > capacity) result.truncated = true;
        rawText += delta.slice(0, capacity);
        break;
      }
      case "text_segment_start":
        fields(payload, ["type", "iteration"]);
        count(payload.iteration);
        if (payload.iteration === 0) throw new Error("Invalid text iteration");
        break;
      case "tool_start":
      case "tool_end": {
        fields(
          payload,
          type === "tool_start"
            ? ["type", "id", "name", "input"]
            : ["type", "id", "name", "result", "is_error"],
        );
        const id = string(payload.id);
        const name = string(payload.name);
        if (!id || id.length > 256 || !/^[A-Za-z0-9_.:-]{1,128}$/.test(name))
          throw new Error("Invalid passive tool identity");
        if (type === "tool_end") {
          string(payload.result);
          if (typeof payload.is_error !== "boolean") throw new Error("Invalid tool result status");
        } else json(payload.input);
        const old = tools.get(id);
        if (old && old.name !== name) throw new Error("Conflicting tool identity");
        if (old && type === "tool_start")
          throw new Error("Tool status regressed or repeated start");
        if (!old && tools.size >= MAX_TOOLS) {
          result.truncated = true;
          break;
        }
        tools.set(id, {
          id,
          name,
          status: type === "tool_start" ? "running" : payload.is_error ? "failed" : "completed",
        });
        break;
      }
      case "error":
        fields(payload, ["type", "message"], ["code", "details"]);
        string(payload.message);
        if (payload.code != null) string(payload.code);
        if (own(payload, "details")) json(payload.details);
        result.errorCount += 1;
        result.outcome = "error";
        break;
      case "done":
        fields(payload, ["type", "total_input_tokens", "total_output_tokens"]);
        count(payload.total_input_tokens);
        count(payload.total_output_tokens);
        if (result.outcome !== "error") result.outcome = "done";
        break;
      case "cancelled":
        fields(payload, ["type"]);
        result.outcome = "cancelled";
        break;
      default: {
        const allowed: Record<string, [string[], string[]]> = {
          message_commit: [["message"], []],
          permission_request: [
            ["request_id", "tool_name", "tool_input", "description"],
            ["approval"],
          ],
          plan_update: [["items"], []],
          interactive_ui: [["request_id", "ui_definition"], []],
          interactive_ui_patch: [["request_id", "patch"], []],
          interactive_ui_listen: [["request_id"], []],
          notice: [["message"], ["code", "details"]],
          context_usage: [
            [
              "estimated_input_tokens",
              "total_input_budget",
              "trigger_threshold",
              "cumulative_input_tokens",
              "cumulative_output_tokens",
              "rolling_summary_version",
              "auto_compact_threshold",
            ],
            ["layered_breakdown"],
          ],
          fish_progress: [
            ["fish_id", "fish_name", "iteration", "tool_name", "status"],
            ["text_delta"],
          ],
        };
        if (!own(allowed, type)) throw new Error("Unknown agent event type");
        fields(payload, ["type", ...allowed[type][0]], allowed[type][1]);
        json(payload);
        for (const field of ["request_id", "description"])
          if (own(payload, field)) string(payload[field]);
        if (own(payload, "tool_name") && (type !== "fish_progress" || payload.tool_name !== null))
          string(payload.tool_name);
        if (type === "notice") {
          string(payload.message);
          if (payload.code != null) string(payload.code);
        }
        if (type === "context_usage") for (const field of allowed[type][0]) count(payload[field]);
        if (type === "plan_update") {
          if (!Array.isArray(payload.items) || payload.items.length > 500)
            throw new Error("Invalid plan snapshot");
          for (const raw of payload.items) {
            const item = object(raw);
            fields(item, ["id", "content", "status"]);
            string(item.id);
            string(item.content);
            if (!["pending", "in_progress", "completed", "cancelled"].includes(string(item.status)))
              throw new Error("Invalid plan item status");
          }
        }
        if (type === "context_usage" && payload.layered_breakdown != null) {
          const layered = object(payload.layered_breakdown);
          const names = [
            "persona",
            "scene",
            "memory",
            "project",
            "platform_hint",
            "tool_defs",
            "history_text",
            "history_tool_result_full",
            "history_tool_result_receipt",
            "rolling_summary",
            "state_frame",
            "vision",
            "request_overhead",
          ];
          fields(layered, names);
          names.forEach((name) => count(layered[name]));
        }
        if (type === "fish_progress") {
          string(payload.fish_id);
          string(payload.fish_name);
          count(payload.iteration);
          string(payload.status);
          if (own(payload, "text_delta")) string(payload.text_delta);
        }
        result.ignoredCount += 1;
      }
    }
  }
  result.tools = [...tools.values()];
  result.text = redacted(rawText);
  return result;
}

export interface StreamContextUsageSnapshot {
  estimatedInputTokens: number;
  totalInputBudget: number;
  triggerThreshold: number;
  cumulativeInputTokens: number;
  cumulativeOutputTokens: number;
  rollingSummaryVersion: number;
  autoCompactThreshold: number;
}

/** Normalize a raw `context_usage` agent event into the shared UI snapshot used by CodeZ & WorkZ. */
export function normalizeStreamContextUsage(evt: {
  estimated_input_tokens: number;
  total_input_budget: number;
  trigger_threshold: number;
  cumulative_input_tokens: number;
  cumulative_output_tokens: number;
  rolling_summary_version: number;
  auto_compact_threshold: number;
}): StreamContextUsageSnapshot {
  return {
    estimatedInputTokens: evt.estimated_input_tokens,
    totalInputBudget: evt.total_input_budget,
    triggerThreshold: evt.trigger_threshold,
    cumulativeInputTokens: evt.cumulative_input_tokens,
    cumulativeOutputTokens: evt.cumulative_output_tokens,
    rollingSummaryVersion: evt.rolling_summary_version,
    autoCompactThreshold: evt.auto_compact_threshold,
  };
}

