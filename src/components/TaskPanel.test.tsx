import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";
import TaskPanel, { mergePlanItems, parsePlanFromToolInput } from "./TaskPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { count?: number; defaultValue?: string }) =>
      opts?.defaultValue ?? (opts?.count !== undefined ? `${key}:${opts.count}` : key),
  }),
}));

describe("TaskPanel", () => {
  it("merges and parses plan items from update_plan tool input", () => {
    const parsed = parsePlanFromToolInput({
      todos: [
        { id: "step-1", content: "Prepare schema", status: "completed" },
        { id: "step-2", content: "Wire UI", status: "in_progress" },
      ],
    });
    expect(parsed).toHaveLength(2);
    const merged = mergePlanItems(parsed, [
      { id: "step-2", content: "Wire UI", status: "completed" },
      { id: "step-3", content: "Run tests", status: "pending" },
    ]);
    expect(merged.map((i) => `${i.id}:${i.status}`)).toEqual([
      "step-1:completed",
      "step-2:completed",
      "step-3:pending",
    ]);
  });

  it("renders persisted plan items with completion ratio and lightweight clear action", () => {
    const onOpenChange = vi.fn();
    const onClearPlan = vi.fn();
    let renderer: ReactTestRenderer | undefined;
    act(() => {
      renderer = create(
        <TaskPanel
          planItems={[
            { id: "1", content: "First step", status: "completed" },
            { id: "2", content: "Second step", status: "in_progress" },
          ]}
          busy={false}
          open={true}
          onOpenChange={onOpenChange}
          onClearPlan={onClearPlan}
        />,
      );
    });
    const text = JSON.stringify(renderer?.toJSON());
    expect(text).toContain("1/2");
    expect(text).toContain("First step");
    expect(text).toContain("Second step");

    const clearBtn = renderer?.root.findByProps({ className: "agentz-task-panel-clear" });
    act(() => {
      clearBtn?.props.onClick();
    });
    expect(onClearPlan).toHaveBeenCalledTimes(1);
  });
});
