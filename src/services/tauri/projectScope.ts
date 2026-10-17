import { invoke } from "@tauri-apps/api/core";
import { useCallback, useSyncExternalStore } from "react";

/** Opaque host authority. Keep this DTO in memory; never persist or add to prompts. */
export interface ProjectScopeDTO {
  readonly handle: string;
  readonly project_dir: string;
}

/** Persist directory_id only; scope handles remain ephemeral host authority. */
export interface ProjectlessDirectoryDTO {
  readonly directory_id: string;
  readonly scope: ProjectScopeDTO;
}
export interface ProjectlessSessionDTO {
  readonly session_id: string;
  readonly session_project_dir: string;
  readonly directory: ProjectlessDirectoryDTO;
}
const directoryIdentity = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

function rememberDirectory(value: unknown, expectedId?: string): ProjectlessDirectoryDTO {
  if (!value || typeof value !== "object" || !("directory_id" in value) ||
      typeof value.directory_id !== "string" || !directoryIdentity.test(value.directory_id) ||
      (expectedId !== undefined && value.directory_id !== expectedId) || !("scope" in value)) {
    throw new Error("Host returned an invalid projectless directory identity");
  }
  return Object.freeze({ directory_id: value.directory_id, scope: remember(value.scope) });
}

/** User-initiated creation only. Failure is propagated; never retry implicitly. */
export async function createProjectlessDirectory(): Promise<ProjectlessDirectoryDTO> {
  return rememberDirectory(await invoke<unknown>("projectless_directory_create"));
}

/** Reopen the durable directory identity; never recreate it on failure. */
export async function reopenProjectlessDirectory(directoryId: string): Promise<ProjectlessDirectoryDTO> {
  if (!directoryIdentity.test(directoryId)) throw new Error("Invalid projectless directory identity");
  return rememberDirectory(await invoke<unknown>("projectless_directory_reopen", {
    directoryId,
  }), directoryId);
}

function rememberSession(value: unknown, expectedId?: string): ProjectlessSessionDTO {
  if (!value || typeof value !== "object" || !("session_id" in value) ||
      typeof value.session_id !== "string" || !directoryIdentity.test(value.session_id) ||
      (expectedId !== undefined && value.session_id !== expectedId) ||
      !("session_project_dir" in value) || typeof value.session_project_dir !== "string" ||
      !value.session_project_dir.trim() || !("directory" in value)) {
    throw new Error("Host returned an invalid projectless session identity");
  }
  return Object.freeze({ session_id: value.session_id, session_project_dir: value.session_project_dir,
    directory: rememberDirectory(value.directory) });
}
/** Creates a host-assigned session identity and durable directory binding before chat. */
export async function createProjectlessSession(): Promise<ProjectlessSessionDTO> {
  return rememberSession(await invoke<unknown>("projectless_session_create"));
}
/** Lost binding/directory is propagated; no legacy cwd or replacement fallback. */
export async function reopenProjectlessSession(sessionId: string): Promise<ProjectlessSessionDTO> {
  if (!directoryIdentity.test(sessionId)) throw new Error("Invalid projectless session identity");
  return rememberSession(await invoke<unknown>("projectless_session_reopen", { sessionId }), sessionId);
}
/** Host lookup distinguishes a legacy free session from a managed one; catalog loss rejects. */
export async function lookupProjectlessSession(sessionId: string): Promise<ProjectlessSessionDTO | null> {
  if (!directoryIdentity.test(sessionId)) return null;
  const value = await invoke<unknown>("projectless_session_lookup", { sessionId });
  return value == null ? null : rememberSession(value, sessionId);
}

export type ProjectScopeRestoreSource = "workspace" | "workz_project" | "workz_free";

const scopes = new Map<string, ProjectScopeDTO>();
const listeners = new Set<() => void>();

export function subscribeProjectScopes(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notifyScopesChanged(): void {
  for (const listener of listeners) listener();
}

function remember(value: unknown): ProjectScopeDTO {
  if (
    !value ||
    typeof value !== "object" ||
    !("handle" in value) ||
    typeof value.handle !== "string" ||
    !value.handle.trim() ||
    !("project_dir" in value) ||
    typeof value.project_dir !== "string" ||
    !value.project_dir.trim()
  ) {
    throw new Error("Host returned an invalid project scope");
  }
  const existing = scopes.get(value.project_dir);
  if (existing?.handle === value.handle) return existing;
  const scope = Object.freeze({ handle: value.handle, project_dir: value.project_dir });
  scopes.set(scope.project_dir, scope);
  notifyScopesChanged();
  return scope;
}

/** Native directory picker; no renderer-supplied path can create a scope. */
export async function pickProjectScope(): Promise<ProjectScopeDTO | null> {
  const picked = await invoke<unknown>("project_scope_pick");
  return picked === null ? null : remember(picked);
}

/** Restore only a host-authorized workspace, saved WorkZ project or free workspace. */
export async function restoreProjectScope(
  source: ProjectScopeRestoreSource,
  path?: string,
): Promise<ProjectScopeDTO> {
  if (source === "workz_project") {
    if (!path?.trim()) throw new Error("Saved WorkZ project identity is required");
  } else if (source !== "workspace" && source !== "workz_free") {
    throw new Error("Unknown project scope restore source");
  } else if (path !== undefined) {
    throw new Error("This restore source does not accept a renderer path");
  }
  return remember(
    await invoke<unknown>("project_scope_restore", {
      source,
      ...(source === "workz_project" ? { path } : {}),
    }),
  );
}

/** Read the latest in-memory handle for the exact canonical path returned by the host. */
export function getProjectScope(projectDir: string): ProjectScopeDTO | null {
  return scopes.get(projectDir) ?? null;
}

const noServerScope = () => null;

/** Observe scope acquisition/revocation even when the selected directory is unchanged. */
export function useProjectScope(projectDir: string | null): ProjectScopeDTO | null {
  const getSnapshot = useCallback(
    () => (projectDir ? getProjectScope(projectDir) : null),
    [projectDir],
  );
  return useSyncExternalStore(subscribeProjectScopes, getSnapshot, noServerScope);
}

/** Forget locally before awaiting revocation; failures cannot keep a usable cached token. */
export async function revokeProjectScope(handle: string): Promise<void> {
  if (!handle.trim()) throw new Error("Project scope handle is required");
  let changed = false;
  for (const [project, scope] of scopes) {
    if (scope.handle === handle) {
      scopes.delete(project);
      changed = true;
    }
  }
  if (changed) notifyScopesChanged();
  await invoke<void>("project_scope_revoke", { handle });
}
