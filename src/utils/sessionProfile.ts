/** Per-session composer selection (model, agent, skills). */

export interface SessionProfile {
  modelId: string;
  agentId: string;
  enabledSkills: string[];
}

const PREFIX = "agentz-session-profile:";

export function loadSessionProfile(sessionId: string): SessionProfile | null {
  try {
    const raw = localStorage.getItem(PREFIX + sessionId);
    if (!raw) return null;
    const p = JSON.parse(raw) as Partial<SessionProfile>;
    return {
      modelId: typeof p.modelId === "string" ? p.modelId : "",
      agentId: typeof p.agentId === "string" ? p.agentId : "",
      enabledSkills: Array.isArray(p.enabledSkills) ? p.enabledSkills.filter((s) => typeof s === "string") : [],
    };
  } catch {
    return null;
  }
}

export function saveSessionProfile(sessionId: string, profile: SessionProfile): void {
  try {
    localStorage.setItem(PREFIX + sessionId, JSON.stringify(profile));
  } catch {
    /* storage full or unavailable */
  }
}

export function removeSessionProfile(sessionId: string): void {
  try {
    localStorage.removeItem(PREFIX + sessionId);
  } catch {
    /* ignore */
  }
}
