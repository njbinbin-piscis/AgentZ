// Opening a project session in CodeZ from WorkZ. App switches the project and mode first, then
// dispatches this event; AssistantPanel applies it once the requested project is current.

export const OPEN_CODEZ_SESSION_EVENT = "agentz:open-codez-session";

export interface OpenCodezSessionDetail {
  projectDir: string;
  sessionId: string;
  source: string | null;
}

export function requestOpenCodezSession(detail: OpenCodezSessionDetail): void {
  window.dispatchEvent(new CustomEvent<OpenCodezSessionDetail>(OPEN_CODEZ_SESSION_EVENT, { detail }));
}

/** Same normalisation as App's project comparison: unify separators, drop trailing slashes. */
export function sameProjectPath(a: string | null | undefined, b: string | null | undefined): boolean {
  if (!a || !b) return false;
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  return norm(a) === norm(b);
}
