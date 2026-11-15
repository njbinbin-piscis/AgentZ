// @vitest-environment jsdom
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import ToolTrace, { type ToolTraceItem } from "./ToolTrace";

const translation = { t: (key: string) => key };
vi.mock("react-i18next", () => ({ useTranslation: () => translation }));

const NOT_STARTED = "Tool execution was cancelled or interrupted before completion.";

const read = (id: string, extra: Partial<ToolTraceItem>): ToolTraceItem => ({
  id,
  name: "file_read",
  input: { path: `docs/${id}.md` },
  status: "done",
  ...extra,
});

afterEach(cleanup);

describe("ToolTrace: calls cut short by Stop", () => {
  const stoppedBatch: ToolTraceItem[] = [
    read("a", { result: "contents" }),
    read("b", { status: "cancelled", result: "已被用户取消" }),
    read("c", { status: "cancelled", skipped: true, result: NOT_STARTED }),
    read("d", { status: "cancelled", skipped: true, result: NOT_STARTED }),
  ];

  it("counts only the calls that ran", () => {
    const { container } = render(<ToolTrace items={stoppedBatch} />);
    // Two calls ran (a, b); the two the stop pre-empted are not counted.
    expect(container.querySelector(".agentz-tool-trace-badge")?.textContent).toBe("2");
    // The collapsed line shows the last call that ran, not a skipped one.
    expect(container.querySelector(".agentz-tool-trace-hint")?.textContent).toContain("b.md");
  });

  it("lists them as cancelled, never as errors", () => {
    const { container } = render(<ToolTrace items={stoppedBatch} />);
    fireEvent.click(container.querySelector(".agentz-tool-trace-head")!);
    const rows = Array.from(container.querySelectorAll(".agentz-tool-trace-row"));
    expect(rows.map((row) => row.className.replace("agentz-tool-trace-row ", ""))).toEqual([
      "is-done",
      "is-cancelled",
      "is-cancelled",
      "is-cancelled",
    ]);
    expect(container.querySelectorAll(".agentz-tool-trace-cancelled")).toHaveLength(3);
    expect(container.querySelector(".is-error")).toBeNull();
  });

  it("does not present the internal cancel text as tool output", () => {
    const { container, queryByText } = render(
      <ToolTrace items={[read("b", { status: "cancelled", result: "已被用户取消" })]} />,
    );
    // First click opens the batch, the second expands the row itself.
    fireEvent.click(container.querySelector(".agentz-tool-trace-head")!);
    fireEvent.click(container.querySelector(".agentz-tool-trace-head")!);
    expect(queryByText("chat.toolStepInput")).not.toBeNull();
    expect(queryByText("chat.toolStepOutput")).toBeNull();
    expect(queryByText("chat.toolStepError")).toBeNull();
  });
});
