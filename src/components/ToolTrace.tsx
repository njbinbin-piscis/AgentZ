import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toolIcon, toolSummary } from "./toolDisplay";
import "./ToolTrace.css";

/** Normalized tool-call record shared by the CodeZ and WorkZ event streams. */
export interface ToolTraceItem {
  id: string;
  name: string;
  input?: unknown;
  result?: string;
  status: "running" | "done" | "error";
  /** Length of the assistant text already streamed when this call started. */
  textOffset?: number;
}

export interface ToolTextSegment {
  text: string;
  /** Tool calls that started right after `text` (in call order). */
  tools: ToolTraceItem[];
}

/**
 * Split streamed assistant text at the point each tool call started, so the UI can
 * render `text -> tools -> text -> tools ...` in the order things actually happened.
 * Tools without an offset (e.g. restored history) trail the text.
 */
export function interleaveTools(text: string, tools: ToolTraceItem[]): ToolTextSegment[] {
  const segments: ToolTextSegment[] = [];
  let cursor = 0;
  let current: ToolTextSegment | null = null;
  for (const tool of tools) {
    const at = Math.min(text.length, Math.max(cursor, tool.textOffset ?? text.length));
    if (at > cursor || !current) {
      if (at > cursor) segments.push({ text: text.slice(cursor, at), tools: [] });
      current = { text: "", tools: [] };
      segments.push(current);
      cursor = at;
    }
    current.tools.push(tool);
  }
  if (cursor < text.length) segments.push({ text: text.slice(cursor), tools: [] });
  if (segments.length === 0) segments.push({ text, tools: [] });
  return segments;
}

/**
 * Tool output is retained by the agent/session store, but never keep an
 * unbounded duplicate in React state. A single verbose command previously
 * allocated enough WebKit memory to kill the renderer.
 */
export const MAX_TOOL_RESULT_CHARS = 64 * 1024;

export function truncateToolResultForUi(result: string): string {
  if (result.length <= MAX_TOOL_RESULT_CHARS) return result;
  const head = Math.floor(MAX_TOOL_RESULT_CHARS * 0.75);
  const tail = MAX_TOOL_RESULT_CHARS - head;
  return `${result.slice(0, head)}\n\n… [UI output truncated: ${result.length.toLocaleString()} characters total] …\n\n${result.slice(-tail)}`;
}

/** Insert or refresh a tool step when `tool_start` arrives (events may repeat). */
export function upsertToolStep(
  prev: ToolTraceItem[],
  evt: { id: string; name: string; input?: unknown },
  textOffset?: number,
): ToolTraceItem[] {
  const idx = prev.findIndex((s) => s.id === evt.id);
  if (idx >= 0) {
    const next = prev.slice();
    next[idx] = { ...next[idx], name: evt.name, input: evt.input };
    return next;
  }
  return [
    ...prev,
    { id: evt.id, name: evt.name, input: evt.input, status: "running", textOffset },
  ];
}

const RESULT_PREVIEW_CHARS = 400;

function ToolTraceRow({ item }: { item: ToolTraceItem }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [showFull, setShowFull] = useState(false);

  const hint = toolSummary(item.name, item.input) || item.name;
  const result = item.result ?? "";
  const truncated = result.length > RESULT_PREVIEW_CHARS;

  return (
    <div className={`agentz-tool-trace-row is-${item.status}`}>
      <button
        type="button"
        className="agentz-tool-trace-head"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
        title={hint}
      >
        <span className="agentz-tool-trace-status">
          {item.status === "running" ? (
            <span className="agentz-tool-trace-spinner" />
          ) : item.status === "error" ? (
            "✕"
          ) : (
            "✓"
          )}
        </span>
        <span className="agentz-tool-trace-icon" aria-hidden>
          {toolIcon(item.name)}
        </span>
        <span className="agentz-tool-trace-name">{item.name}</span>
        <span className="agentz-tool-trace-hint">{hint}</span>
        <span className="agentz-tool-trace-chevron" aria-hidden>
          {expanded ? "▾" : "▸"}
        </span>
      </button>
      {!expanded && item.status !== "running" && result && (
        <div className={`agentz-tool-trace-preview${item.status === "error" ? " is-error" : ""}`}>
          {result.replace(/\s+/g, " ").trim().slice(0, 160)}
        </div>
      )}
      {expanded && (
        <div className="agentz-tool-trace-body">
          <div className="agentz-tool-trace-section">
            <span className="agentz-tool-trace-label">{t("chat.toolStepInput")}</span>
            <pre className="agentz-tool-trace-pre">
              {typeof item.input === "string"
                ? item.input
                : JSON.stringify(item.input ?? {}, null, 2)}
            </pre>
          </div>
          {item.status !== "running" && result && (
            <div className="agentz-tool-trace-section">
              <span
                className={`agentz-tool-trace-label${item.status === "error" ? " is-error" : ""}`}
              >
                {item.status === "error" ? t("chat.toolStepError") : t("chat.toolStepOutput")}
              </span>
              <pre className={`agentz-tool-trace-pre${item.status === "error" ? " is-error" : ""}`}>
                {showFull || !truncated ? result : `${result.slice(0, RESULT_PREVIEW_CHARS)}…`}
              </pre>
              {truncated && (
                <button
                  type="button"
                  className="agentz-tool-trace-more"
                  onClick={() => setShowFull((v) => !v)}
                >
                  {showFull ? t("chat.toolStepShowLess") : t("chat.toolStepShowMore")}
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/**
 * Inline, default-collapsed trace of the tool calls for one assistant turn.
 * Each row expands individually; the active call leads with a spinner.
 */
export default function ToolTrace({ items }: { items: ToolTraceItem[] }) {
  if (items.length === 0) return null;
  return (
    <div className="agentz-tool-trace">
      {items.map((item) => (
        <ToolTraceRow key={item.id} item={item} />
      ))}
    </div>
  );
}
