import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
let api: typeof import("./projectScope");
beforeEach(async () => {
  vi.resetModules();
  invoke.mockReset();
  api = await import("./projectScope");
});
describe("host-authorized project scopes", () => {
  it("keeps host-assigned session identity distinct from directory identity and scope", async () => {
    const session = "11111111-2222-3333-4444-555555555555";
    const directory = "00000000-2222-3333-4444-555555555555";
    invoke.mockResolvedValue({ session_id: session, session_project_dir: "free-db", directory: {
      directory_id: directory, scope: { handle: "opened", project_dir: "owned" },
    } });
    const created = await api.createProjectlessSession();
    expect(invoke).toHaveBeenLastCalledWith("projectless_session_create");
    expect(created.session_id).toBe(session);
    expect(created.directory.directory_id).toBe(directory);
    expect(created.session_project_dir).toBe("free-db");
    expect(api.getProjectScope("owned")).toBe(created.directory.scope);
    expect(Object.isFrozen(created)).toBe(true);
    expect(await api.reopenProjectlessSession(session)).toEqual(created);
    expect(invoke).toHaveBeenLastCalledWith("projectless_session_reopen", { sessionId: session });
  });
  it("rejects mismatched session/root/directory before publishing scope", async () => {
    const session = "11111111-2222-3333-4444-555555555555";
    const good = { session_id: session, session_project_dir: "free-db", directory: {
      directory_id: session, scope: { handle: "opened", project_dir: "owned" },
    } };
    const listener = vi.fn();
    api.subscribeProjectScopes(listener);
    await expect(api.reopenProjectlessSession("../escape")).rejects.toThrow("identity");
    expect(invoke).not.toHaveBeenCalled();
    for (const invalid of [{ ...good, session_id: "00000000-2222-3333-4444-555555555555" },
      { ...good, session_project_dir: " " }, { ...good, directory: { ...good.directory, directory_id: "bad" } }]) {
      invoke.mockResolvedValue(invalid);
      await expect(api.reopenProjectlessSession(session)).rejects.toThrow("identity");
    }
    expect(api.getProjectScope("owned")).toBeNull();
    expect(listener).not.toHaveBeenCalled();
  });
  it("does not retry or replace a lost managed session binding", async () => {
    invoke.mockRejectedValue(new Error("binding store requires recovery"));
    await expect(api.reopenProjectlessSession("11111111-2222-3333-4444-555555555555")).rejects.toThrow("recovery");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe("projectless_session_reopen");
  });
  it("distinguishes legacy sessions while preserving catalog-loss errors", async () => {
    const id = "11111111-2222-3333-4444-555555555555";
    expect(await api.lookupProjectlessSession("legacy-session")).toBeNull();
    expect(invoke).not.toHaveBeenCalled();
    invoke.mockResolvedValueOnce(null);
    expect(await api.lookupProjectlessSession(id)).toBeNull();
    expect(invoke).toHaveBeenLastCalledWith("projectless_session_lookup", { sessionId: id });
    invoke.mockRejectedValueOnce(new Error("catalog requires recovery"));
    await expect(api.lookupProjectlessSession(id)).rejects.toThrow("recovery");
  });
  it("acquires projectless scope and reopens with only the durable directory ID", async () => {
    const id = "11111111-2222-3333-4444-555555555555";
    invoke.mockResolvedValue({ directory_id: id, scope: { handle: "first", project_dir: "owned" } });
    const created = await api.createProjectlessDirectory();
    expect(invoke).toHaveBeenLastCalledWith("projectless_directory_create");
    expect(api.getProjectScope("owned")).toBe(created.scope);
    expect(Object.isFrozen(created)).toBe(true);
    invoke.mockResolvedValue({ directory_id: id, scope: { handle: "restored", project_dir: "owned" } });
    const reopened = await api.reopenProjectlessDirectory(id);
    expect(invoke).toHaveBeenLastCalledWith("projectless_directory_reopen", { directoryId: id });
    expect(reopened.directory_id).toBe(id);
    expect(api.getProjectScope("owned")).toBe(reopened.scope);
  });
  it("rejects invalid or mismatched directory identities before caching authority", async () => {
    const id = "11111111-2222-3333-4444-555555555555";
    const listener = vi.fn();
    api.subscribeProjectScopes(listener);
    for (const invalid of ["../escape", "", id.toUpperCase().replace("111", "ABC")]) {
      await expect(api.reopenProjectlessDirectory(invalid)).rejects.toThrow("identity");
    }
    expect(invoke).not.toHaveBeenCalled();
    invoke.mockResolvedValue({ directory_id: "00000000-2222-3333-4444-555555555555", scope: { handle: "wrong", project_dir: "owned" } });
    await expect(api.reopenProjectlessDirectory(id)).rejects.toThrow("identity");
    invoke.mockResolvedValue({ directory_id: id, scope: { handle: "", project_dir: "owned" } });
    await expect(api.createProjectlessDirectory()).rejects.toThrow("scope");
    expect(api.getProjectScope("owned")).toBeNull();
    expect(listener).not.toHaveBeenCalled();
  });
  it("propagates missing directory failures without creating a replacement", async () => {
    invoke.mockRejectedValue(new Error("registered directory missing"));
    await expect(api.reopenProjectlessDirectory("11111111-2222-3333-4444-555555555555")).rejects.toThrow("missing");
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][0]).toBe("projectless_directory_reopen");
  });
  it("publishes stable observable snapshots for acquisition, replacement and revocation", async () => {
    const listener = vi.fn(() => api.getProjectScope("project"));
    const unsubscribe = api.subscribeProjectScopes(listener);
    expect(api.getProjectScope("project")).toBeNull();
    invoke.mockResolvedValueOnce({ handle: "first", project_dir: "project" });
    const first = await api.pickProjectScope();
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener.mock.results[0].value).toBe(first);
    expect(api.getProjectScope("project")).toBe(first);
    invoke.mockResolvedValueOnce({ handle: "first", project_dir: "project" });
    expect(await api.restoreProjectScope("workspace")).toBe(first);
    expect(listener).toHaveBeenCalledTimes(1);
    invoke.mockResolvedValueOnce(null);
    await api.pickProjectScope();
    expect(listener).toHaveBeenCalledTimes(1);
    invoke.mockResolvedValueOnce({ handle: "second", project_dir: "project" });
    const second = await api.pickProjectScope();
    expect(listener).toHaveBeenCalledTimes(2);
    expect(listener.mock.results[1].value).toBe(second);
    invoke.mockResolvedValueOnce(undefined);
    await api.revokeProjectScope("first");
    expect(listener).toHaveBeenCalledTimes(2);
    invoke.mockRejectedValueOnce(new Error("offline"));
    const revocation = api.revokeProjectScope("second");
    // Local revocation is observable before the native acknowledgement, including failure.
    expect(listener).toHaveBeenCalledTimes(3);
    expect(listener.mock.results[2].value).toBeNull();
    await expect(revocation).rejects.toThrow("offline");
    unsubscribe();
    invoke.mockResolvedValueOnce({ handle: "third", project_dir: "project" });
    await api.pickProjectScope();
    expect(listener).toHaveBeenCalledTimes(3);
  });
  it("uses the native picker without path/window/generation arguments and stores immutable DTOs", async () => {
    invoke.mockResolvedValue({ handle: "opaque", project_dir: "canonical-project" });
    const scope = await api.pickProjectScope();
    expect(invoke).toHaveBeenCalledWith("project_scope_pick");
    expect(api.getProjectScope("canonical-project")).toBe(scope);
    expect(Object.isFrozen(scope)).toBe(true);
    expect(api.getProjectScope("arbitrary-renderer-path")).toBeNull();
    invoke.mockResolvedValue(null);
    expect(await api.pickProjectScope()).toBeNull();
    expect(api.getProjectScope("canonical-project")).toBe(scope);
  });
  it("restores fixed host sources and sends a path only for a saved WorkZ project", async () => {
    invoke.mockResolvedValue({ handle: "restore", project_dir: "canonical-project" });
    await api.restoreProjectScope("workspace");
    expect(invoke).toHaveBeenLastCalledWith("project_scope_restore", { source: "workspace" });
    await api.restoreProjectScope("workz_free");
    expect(invoke).toHaveBeenLastCalledWith("project_scope_restore", { source: "workz_free" });
    await api.restoreProjectScope("workz_project", "saved-project");
    expect(invoke).toHaveBeenLastCalledWith("project_scope_restore", {
      source: "workz_project",
      path: "saved-project",
    });
    invoke.mockClear();
    await expect(api.restoreProjectScope("workspace", "arbitrary-path")).rejects.toThrow(
      "does not accept",
    );
    await expect(api.restoreProjectScope("workz_free", "arbitrary-path")).rejects.toThrow(
      "does not accept",
    );
    await expect(api.restoreProjectScope("workz_project")).rejects.toThrow("identity is required");
    await expect(api.restoreProjectScope("unknown" as never)).rejects.toThrow("Unknown");
    expect(invoke).not.toHaveBeenCalled();
  });
  it("refuses malformed host responses and propagates authorization errors without fallback mint", async () => {
    invoke.mockResolvedValue({ handle: "", project_dir: "path" });
    await expect(api.pickProjectScope()).rejects.toThrow("invalid project scope");
    expect(api.getProjectScope("path")).toBeNull();
    invoke.mockRejectedValue(new Error("project is not in host saved projects"));
    await expect(api.restoreProjectScope("workz_project", "foreign")).rejects.toThrow(
      "not in host",
    );
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it("forgets revoked scopes on failure and preserves newer scopes when an older handle is revoked", async () => {
    invoke.mockResolvedValueOnce({ handle: "old", project_dir: "project" });
    await api.pickProjectScope();
    invoke.mockResolvedValueOnce({ handle: "new", project_dir: "project" });
    await api.pickProjectScope();
    invoke.mockResolvedValueOnce(undefined);
    await api.revokeProjectScope("old");
    expect(api.getProjectScope("project")?.handle).toBe("new");
    invoke.mockRejectedValueOnce(new Error("host unavailable"));
    await expect(api.revokeProjectScope("new")).rejects.toThrow("host unavailable");
    expect(api.getProjectScope("project")).toBeNull();
    expect(invoke).toHaveBeenLastCalledWith("project_scope_revoke", { handle: "new" });
  });
});
