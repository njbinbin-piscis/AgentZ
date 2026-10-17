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
vi.mock("./DeliverySummary", () => ({
  default: ({ run, artifacts }: Record<string, unknown>) => (
    <pre>{JSON.stringify({ run, artifacts })}</pre>
  ),
}));
import DeliveryReviewPanel from "./DeliveryReviewPanel";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", onOpenArtifacts: vi.fn() };
const binding = { project_id: "project", task_id: "task", environment_id: "env" };
const detail = {
  scope: binding,
  sources: {
    run: { status: "available", value: { execution: "succeeded" }, version: "v" },
    artifacts: { status: "unsupported" },
  },
};
const page = {
  scope: binding,
  store_missing: false,
  page: {
    versions: [{ kind: "validation", title: "attachment" }],
    has_more: true,
    next_cursor: ["id", 1],
  },
};
const output = () => JSON.stringify(renderer?.toJSON());
beforeEach(() => {
  details.mockReset();
  observe.mockReset();
  list.mockReset();
  scopes.clear();
  scopes.set("project", { handle: "opaque", project_dir: "project" });
  details.mockResolvedValue(detail);
  observe.mockResolvedValue({ session_id: "task" });
  list.mockResolvedValue(page);
});
afterEach(async () => {
  await act(async () => renderer?.unmount());
  renderer = undefined;
});
describe("DeliveryReviewPanel", () => {
  it("observes a separate bounded artifact source without changing unsupported taskDetails", async () => {
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    expect(details).toHaveBeenCalledWith("opaque", "task");
    expect(observe).toHaveBeenCalledWith("opaque", "task", "project");
    expect(list).toHaveBeenCalledWith("opaque", { session_id: "task" }, null, 200);
    expect(output()).toContain("has_more");
    expect(detail.sources.artifacts.status).toBe("unsupported");
  });
  it("keeps artifact failures unavailable rather than an empty success", async () => {
    list.mockRejectedValue(new Error("private path"));
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    expect(output()).toContain("unavailable");
    expect(output()).not.toContain("private path");
    expect(output()).toContain("succeeded");
  });
  it("hides old task observations and ignores late artifact replies", async () => {
    let resolve!: (v: unknown) => void;
    list
      .mockImplementationOnce(
        () =>
          new Promise((r) => {
            resolve = r;
          }),
      )
      .mockResolvedValue({ ...page, scope: { ...binding, task_id: "next" } });
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    details.mockResolvedValue({
      ...detail,
      scope: { ...binding, task_id: "next" },
      sources: { run: { status: "missing" } },
    });
    await act(async () => {
      renderer?.update(<DeliveryReviewPanel {...props} sessionId="next" />);
    });
    await act(async () => {
      resolve({ ...page, page: { ...page.page, versions: [{ title: "OLD_PRIVATE" }] } });
    });
    expect(output()).not.toContain("OLD_PRIVATE");
    expect(output()).not.toContain("succeeded");
  });
  it("marks independently mismatched environment stale and performs no reads without scope", async () => {
    list.mockResolvedValue({ ...page, scope: { ...binding, environment_id: "other" } });
    await act(async () => {
      renderer = create(<DeliveryReviewPanel {...props} />);
    });
    expect(output()).toContain("stale");
    scopes.clear();
    await act(async () => {
      renderer?.update(<DeliveryReviewPanel {...props} />);
    });
    expect(details).toHaveBeenCalledTimes(1);
    expect(output()).toContain("unavailable");
  });
});
