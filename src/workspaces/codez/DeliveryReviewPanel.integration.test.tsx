import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
const { details, observe, list, scopes } = vi.hoisted(() => ({
  details: vi.fn(),
  observe: vi.fn(),
  list: vi.fn(),
  scopes: new Map<string, { handle: string; project_dir: string }>(),
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: "en" } }) }));
vi.mock("../../services/tauri/projectScope", () => ({
  useProjectScope: (p: string) => scopes.get(p) ?? null,
}));
vi.mock("../../services/tauri/taskDetails", () => ({ readTaskDetails: details }));
vi.mock("../../services/tauri/taskView", () => ({ observeTaskView: observe }));
vi.mock("../../services/tauri/taskArtifacts", () => ({ listTaskArtifacts: list }));
import DeliveryReviewPanel from "./DeliveryReviewPanel";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", onOpenArtifacts: vi.fn() };
const binding = { project_id: "project", task_id: "task", environment_id: "env" };
const detail = (execution = "succeeded", delivery = "awaiting_review") => ({
  scope: binding,
  sources: {
    run: { status: "available", version: "v", value: { execution, delivery } },
    artifacts: { status: "unsupported" },
  },
});
const page = {
  scope: binding,
  store_missing: false,
  page: {
    versions: [{ artifact_id: "validation", version: 1, kind: "validation" }],
    has_more: true,
    next_cursor: ["validation", 1],
  },
};
const output = () => JSON.stringify(renderer?.toJSON());
beforeEach(() => {
  details.mockReset();
  observe.mockReset();
  list.mockReset();
  scopes.clear();
  scopes.set("project", { handle: "opaque", project_dir: "project" });
  details.mockResolvedValue(detail());
  observe.mockResolvedValue({ session_id: "task" });
  list.mockResolvedValue(page);
});
afterEach(async () => {
  await act(async () => renderer?.unmount());
  renderer = undefined;
});
describe("DeliveryReviewPanel with real DeliverySummary", () => {
  it("separates succeeded awaiting-review execution, validation attachments and absent receipts", async () => {
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    const shown = output();
    expect(shown).toContain("Succeeded");
    expect(shown).toContain("Awaiting review");
    expect(shown).toContain("more available");
    expect(shown).toContain("does not establish a passed validation");
    expect(shown.match(/No receipt provided/g)).toHaveLength(3);
    expect(renderer!.root.findAllByType("button").map((b) => b.children.join(""))).toEqual([
      "Refresh delivery observations",
      "View artifacts",
    ]);
  });
  it("shows artifact read failure unavailable with unknown validation count rather than zero", async () => {
    list.mockRejectedValue(new Error("private host path"));
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    expect(output()).toContain("Unavailable");
    expect(output()).toContain("Unknown");
    expect(output()).not.toContain("Artifacts shown");
    expect(output()).not.toContain("private host path");
    expect(output()).toContain("Succeeded");
  });
  it("drops late source success after a task switch instead of restoring old execution", async () => {
    let resolve!: (value: unknown) => void;
    details.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    details.mockResolvedValue({
      ...detail("failed", "not_ready"),
      scope: { ...binding, task_id: "next" },
    });
    list.mockResolvedValue({ ...page, scope: { ...binding, task_id: "next" } });
    await act(async () => {
      renderer?.update(<DeliveryReviewPanel {...props} sessionId="next" />);
    });
    expect(output()).toContain("Failed");
    expect(output()).not.toContain("Succeeded");
    await act(async () => {
      resolve(detail());
    });
    expect(output()).toContain("Failed");
    expect(output()).not.toContain("Succeeded");
  });
});
