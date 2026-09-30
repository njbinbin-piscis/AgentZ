import { memo } from "react";
import { useTranslation } from "react-i18next";
import type { PlanTodoItem } from "../services/tauri/chat";
import "./TaskPanel.css";

export function mergePlanItems(existing: PlanTodoItem[], updates: PlanTodoItem[]): PlanTodoItem[] {
  const merged = existing.slice();
  for (const update of updates) {
    const idx = merged.findIndex((item) => item.id === update.id);
    if (idx >= 0) merged[idx] = update;
    else merged.push(update);
  }
  return merged;
}

export function parsePlanFromToolInput(input: unknown): PlanTodoItem[] {
  if (!input || typeof input !== "object") return [];
  const raw = input as { todos?: unknown[] };
  if (!Array.isArray(raw.todos)) return [];
  return raw.todos
    .map((item) => {
      const row = item as { id?: string; content?: string; status?: string };
      return {
        id: row.id ?? "",
        content: row.content ?? "",
        status: row.status ?? "pending",
      };
    })
    .filter((item) => item.id && item.content);
}

function planStatusLabel(t: ReturnType<typeof useTranslation>["t"], status: string): string {
  switch (status) {
    case "pending":
      return t("chat.planPending");
    case "in_progress":
      return t("chat.planInProgress");
    case "completed":
      return t("chat.planCompleted");
    case "cancelled":
      return t("chat.planCancelled");
    default:
      return status;
  }
}

export function PlanPanel({ items }: { items: PlanTodoItem[] }) {
  const { t } = useTranslation();
  return (
    <div className="agentz-plan-panel-inner">
      {items.map((item, index) => (
        <div key={item.id} className={`agentz-plan-item plan-${item.status}`}>
          <div className="agentz-plan-item-left">
            <span className="agentz-plan-item-index">{index + 1}</span>
            <span className="agentz-plan-item-content">{item.content}</span>
          </div>
          <div className="agentz-plan-item-right">
            <span className="agentz-plan-item-id">{item.id}</span>
            <span className={`agentz-plan-item-status plan-status-${item.status}`}>
              {item.status === "in_progress" && <span className="agentz-step-spinner" />}
              {planStatusLabel(t, item.status)}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

export interface TaskPanelProps {
  planItems: PlanTodoItem[];
  busy: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Extra class for layout tweaks (e.g. agent vs IDE spacing). */
  className?: string;
}

/**
 * Collapsible Todo panel. Tool calls render inline in the message stream
 * (see {@link ToolTrace}), so this panel is purely the task plan: it hides
 * when there is no plan and never auto-expands.
 */
function TaskPanel({ planItems, busy, open, onOpenChange, className }: TaskPanelProps) {
  const { t } = useTranslation();

  if (planItems.length === 0) return null;

  return (
    <div className={`agentz-task-panel${className ? ` ${className}` : ""}`}>
      <button
        type="button"
        className="agentz-task-panel-header"
        onClick={() => onOpenChange(!open)}
        aria-expanded={open}
      >
        <div className="agentz-task-panel-title">
          <span className="agentz-task-panel-label">{t("chat.taskPanel")}</span>
          <span className="agentz-task-badge">
            Todo · {busy ? t("chat.planWorking", { count: planItems.length }) : planItems.length}
          </span>
        </div>
        <span className="agentz-task-chevron">{open ? "▲" : "▼"}</span>
      </button>
      {open && (
        <div className="agentz-task-panel-body">
          <PlanPanel items={planItems} />
        </div>
      )}
    </div>
  );
}

export default memo(TaskPanel);
