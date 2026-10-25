import { chatSend, onChatEvent, SESSION_SOURCE_WORKZ, type AgentEvent } from "../../services/tauri/chat";
import { botzGetBot, botzHomeDir, botzSaveBot, type BotProfile, type BotRoutine } from "../../services/tauri/botz";

export const localDate = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** Whether a routine should fire at `now`. Daily routines fire once per day after their time. */
export function isRoutineDue(r: BotRoutine, now: Date): boolean {
  if (!r.enabled || !r.prompt.trim()) return false;
  const last = r.last_run_at ? new Date(r.last_run_at) : null;
  if (r.kind === "interval") {
    const minutes = Math.max(1, r.minutes ?? 60);
    return !last || now.getTime() - last.getTime() >= minutes * 60_000;
  }
  const m = /^(\d{1,2}):(\d{2})$/.exec((r.time ?? "").trim());
  if (!m) return false;
  const slot = new Date(now);
  slot.setHours(Number(m[1]), Number(m[2]), 0, 0);
  return now >= slot && (!last || last < slot);
}

/** Tokens used today, resetting the counter when the day changes. */
export function usageToday(p: BotProfile, today: string): number {
  return p.usage_date === today ? p.usage_tokens : 0;
}

export function overBudget(p: BotProfile, today: string): boolean {
  return p.daily_token_budget > 0 && usageToday(p, today) >= p.daily_token_budget;
}

export function addUsage(p: BotProfile, today: string, tokens: number): BotProfile {
  return { ...p, usage_date: today, usage_tokens: usageToday(p, today) + Math.max(0, tokens) };
}

const MEMORY_RE = /<memory>([\s\S]*?)<\/memory>/i;

/** Pull a `<memory>…</memory>` rewrite out of a reply. */
export function extractMemoryUpdate(reply: string): string | null {
  const m = MEMORY_RE.exec(reply);
  return m ? m[1].trim() : null;
}

export function buildBotPrompt(memory: string, text: string, routine?: string): string {
  const mem = memory.trim();
  return [
    "## Your long-term memory",
    mem || "(empty)",
    "",
    "To change your memory, end your reply with <memory>the full new memory</memory>. Keep it short: facts about the user, ongoing goals and decisions worth remembering across sessions.",
    routine ? `\n## Scheduled routine: ${routine}` : "",
    "",
    "## Message",
    text,
  ].join("\n");
}

const busyBots = new Set<string>();
export const isBotBusy = (id: string) => busyBots.has(id);

export interface BotSendResult {
  ok: boolean;
  reply: string;
  profile: BotProfile;
  error?: string;
}

/**
 * Run one turn in the bot's home thread: memory injected, token usage counted
 * against the daily budget, `<memory>` rewrites persisted.
 */
export async function sendToBot(opts: {
  agentId: string;
  text: string;
  routine?: BotRoutine;
  preferZh?: boolean;
  taskKey?: string;
}): Promise<BotSendResult> {
  const { agentId, text, routine } = opts;
  if (busyBots.has(agentId)) {
    return { ok: false, reply: "", profile: await botzGetBot(agentId), error: "busy" };
  }
  let profile = await botzGetBot(agentId);
  const today = localDate(new Date());
  if (overBudget(profile, today)) {
    return { ok: false, reply: "", profile, error: "budget" };
  }
  busyBots.add(agentId);
  let unlisten: (() => void) | undefined;
  try {
    const homeDir = await botzHomeDir();
    if (!profile.home_session_id) {
      profile = await botzSaveBot({ ...profile, home_session_id: crypto.randomUUID() });
    }
    const sessionId = profile.home_session_id!;
    let tokens = 0;
    unlisten = await onChatEvent((env) => {
      if (env.sessionId !== sessionId || env.channel !== "agent_event") return;
      const evt = env.payload as AgentEvent;
      if (evt.type === "done") tokens += evt.total_input_tokens + evt.total_output_tokens;
    });
    const res = await chatSend({
      prompt: buildBotPrompt(profile.memory, text, routine?.name),
      displayPrompt: routine ? `⏰ ${routine.name}\n${text}` : text,
      sessionId,
      projectDir: homeDir,
      chatMode: "agent",
      clearPlan: false,
      taskKey: opts.taskKey ?? null,
      agentId,
      sessionSource: SESSION_SOURCE_WORKZ,
      preferZh: opts.preferZh ?? null,
    });
    const fresh = await botzGetBot(agentId);
    let next = addUsage(fresh, today, tokens);
    const mem = extractMemoryUpdate(res.response_text);
    if (mem !== null) next = { ...next, memory: mem };
    if (routine) {
      next = {
        ...next,
        routines: next.routines.map((r) => (r.id === routine.id ? { ...r, last_run_at: new Date().toISOString() } : r)),
      };
    }
    next = await botzSaveBot(next);
    return { ok: res.ok, reply: res.response_text, profile: next };
  } catch (e) {
    if (routine) {
      // Record the attempt so a failing routine waits for its next slot instead of retrying every tick.
      const fresh = await botzGetBot(agentId).catch(() => profile);
      profile = await botzSaveBot({
        ...fresh,
        routines: fresh.routines.map((r) => (r.id === routine.id ? { ...r, last_run_at: new Date().toISOString() } : r)),
      }).catch(() => fresh);
    }
    return { ok: false, reply: "", profile, error: String(e) };
  } finally {
    unlisten?.();
    busyBots.delete(agentId);
  }
}
