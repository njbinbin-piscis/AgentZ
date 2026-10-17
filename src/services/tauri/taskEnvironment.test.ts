import { beforeEach, describe, expect, it, vi } from "vitest";
const { get, create } = vi.hoisted(() => ({ get: vi.fn(), create: vi.fn() }));
vi.mock("./agentTask", () => ({ agentTaskApi: { get, create } }));
import { MISSING_TASK_ENVIRONMENT, resolveTaskWorktree } from "./taskEnvironment";
const task = (id: string) => ({
  id,
  environment_id: `env-${id}`,
  worktree_path: `/tasks/${id}`,
  branch: `workz/task-${id}`,
  base: "main",
  base_sha: "base",
});
const args = (id: string) => ({
  projectDir: "/project",
  taskId: id,
  isolate: true,
  isFree: false,
  resuming: true,
});
beforeEach(() => {
  get.mockReset();
  create.mockReset();
});
describe("durable task worktree routing", () => {
  it("passes the explicit start reference and local-source choice for a fresh task", async () => {
    get.mockResolvedValue(null);
    create.mockResolvedValue(task("new"));
    await resolveTaskWorktree({
      ...args("new"),
      resuming: false,
      startMode: "commit",
      base: "release",
    });
    expect(create).toHaveBeenCalledWith("/project", "new", "release", "commit", true);
    await resolveTaskWorktree({ ...args("new"), resuming: false, startMode: "working" });
    expect(create).toHaveBeenLastCalledWith("/project", "new", undefined, "working", false);
  });
  it("routes A/B/A turns to each saved environment", async () => {
    get.mockImplementation(async (_project, id) => task(id));
    const roots = [];
    for (const id of ["a", "b", "a"])
      roots.push((await resolveTaskWorktree(args(id)))?.worktree_path);
    expect(roots).toEqual(["/tasks/a", "/tasks/b", "/tasks/a"]);
    expect(create).not.toHaveBeenCalled();
  });
  it("restores persisted identity without any cached view state", async () => {
    get.mockResolvedValue(task("a"));
    expect(await resolveTaskWorktree({ ...args("a"), isolate: false })).toEqual(task("a"));
    expect(create).not.toHaveBeenCalled();
  });
  it("creates a new isolated task using its session UUID", async () => {
    get.mockResolvedValue(null);
    create.mockResolvedValue(task("uuid"));
    await resolveTaskWorktree({ ...args("uuid"), resuming: false });
    expect(create).toHaveBeenCalledWith("/project", "uuid", undefined, undefined, false);
  });
  it("refuses unbound isolated history instead of silently creating another environment", async () => {
    get.mockResolvedValue(null);
    await expect(resolveTaskWorktree(args("legacy"))).rejects.toThrow(MISSING_TASK_ENVIRONMENT);
    expect(create).not.toHaveBeenCalled();
  });
  it("does not create isolation for free or local tasks", async () => {
    get.mockResolvedValue(null);
    expect(await resolveTaskWorktree({ ...args("local"), isolate: false })).toBeNull();
    expect(await resolveTaskWorktree({ ...args("free"), isFree: true })).toBeNull();
    expect(create).not.toHaveBeenCalled();
  });
  it("propagates invalid environment errors without fallback", async () => {
    get.mockRejectedValue(new Error("registered directory missing"));
    await expect(resolveTaskWorktree(args("a"))).rejects.toThrow("directory missing");
    expect(create).not.toHaveBeenCalled();
  });
});
