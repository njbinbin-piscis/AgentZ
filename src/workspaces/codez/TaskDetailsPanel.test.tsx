import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { read, scopes } = vi.hoisted(() => ({ read: vi.fn(), scopes: new Map<string, { handle: string; project_dir: string }>() }));
vi.mock("../../services/tauri/taskDetails", () => ({ readTaskDetails: read }));
vi.mock("../../services/tauri/projectScope", () => ({ useProjectScope: (project: string | null) => project ? scopes.get(project) ?? null : null }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: "en" } }) }));
import TaskDetailsPanel from "./TaskDetailsPanel";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", busy: false };
const dto = (execution = "running") => ({ sources: {
  run: { status: "available", value: { execution } },
  plan: { status: "available", value: { status: "cancelled", items: [{ state: "done" }, { state: "pending" }] } },
  environment: { status: "missing" }, events: { status: "unknown" }, artifacts: { status: "unsupported" }, budget: { status: "unknown" },
} });
const output = () => JSON.stringify(renderer?.toJSON());
beforeEach(() => { read.mockReset(); scopes.clear(); scopes.set("project", { handle: "opaque", project_dir: "project" }); vi.stubGlobal("window", new EventTarget()); });
afterEach(async () => { await act(async () => renderer?.unmount()); renderer = undefined; vi.unstubAllGlobals(); });
describe("TaskDetailsPanel", () => {
  it("retains cancelled progress and does not invent a budget balance", async () => {
    read.mockResolvedValue(dto()); await act(async () => { renderer = create(<TaskDetailsPanel {...props} />); });
    expect(output()).toContain("Cancelled"); expect(output()).toContain("1"); expect(output()).toContain("no remaining balance"); expect(read).toHaveBeenCalledWith("opaque", "task");
  });
  it("masks old scope on first render and ignores its late reply", async () => {
    let resolve!: (value: unknown) => void;
    read.mockImplementationOnce(() => new Promise(r => { resolve = r; })).mockResolvedValue(dto("failed"));
    await act(async () => { renderer = create(<TaskDetailsPanel {...props} />); });
    await act(async () => { renderer?.update(<TaskDetailsPanel {...props} sessionId="next" />); });
    expect(output()).toContain("Failed"); await act(async () => { resolve(dto("succeeded")); }); expect(output()).not.toContain("Succeeded");
  });
  it("clears prior data on refresh failure and does not display raw error", async () => {
    read.mockResolvedValueOnce(dto()).mockRejectedValueOnce(new Error("private-host-path"));
    await act(async () => { renderer = create(<TaskDetailsPanel {...props} />); });
    await act(async () => { renderer?.root.findByType("button").props.onClick(); });
    expect(output()).toContain("Could not read"); expect(output()).not.toContain("Running"); expect(output()).not.toContain("private-host-path");
  });
  it("does not read without an opaque scope and safely displays unknown state names", async () => {
    scopes.clear(); await act(async () => { renderer = create(<TaskDetailsPanel {...props} />); }); expect(read).not.toHaveBeenCalled();
    scopes.set("project", { handle: "new", project_dir: "project" }); read.mockResolvedValue(dto("__proto__"));
    await act(async () => { renderer?.update(<TaskDetailsPanel {...props} busy />); }); expect(output()).toContain("Unknown");
  });
  it("coalesces focus events during a request into one fresh read and removes listeners", async () => {
    let resolve!: (value: unknown) => void;
    read.mockImplementationOnce(() => new Promise(r => { resolve = r; })).mockResolvedValue(dto("succeeded"));
    await act(async () => { renderer = create(<TaskDetailsPanel {...props} />); });
    await act(async () => { window.dispatchEvent(new Event("focus")); window.dispatchEvent(new Event("focus")); });
    expect(read).toHaveBeenCalledTimes(1);
    await act(async () => { resolve(dto("running")); });
    expect(read).toHaveBeenCalledTimes(2); expect(output()).toContain("Succeeded"); expect(output()).not.toContain("Running");
    await act(async () => { renderer?.unmount(); }); renderer = undefined;
    window.dispatchEvent(new Event("focus")); expect(read).toHaveBeenCalledTimes(2);
  });
});
