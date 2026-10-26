import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { chatCancel, getMessages, onChatEvent, type AgentEvent, type MessageDto } from "../../services/tauri/chat";
import type { AgentInfo } from "../../services/tauri/agents";
import { botzGetBot, botzHomeDir, botzSaveBot, type BotProfile, type BotRoutine } from "../../services/tauri/botz";
import Markdown from "../codez/Markdown";
import Avatar from "./Avatar";
import { budgetLevel, localDate, sendToBot, usageMonth, usageToday } from "./botRuntime";
import type { Activity } from "./activity";

const LIVE_LIMIT = 8000;

function newRoutine(): BotRoutine {
  return { id: crypto.randomUUID(), name: "", prompt: "", kind: "daily", time: "09:00", minutes: 60, enabled: true };
}

export default function BotHome({ agent, activity, onBack }: { agent: AgentInfo; activity: Activity; onBack: () => void }) {
  const { t, i18n } = useTranslation();
  const [profile, setProfile] = useState<BotProfile | null>(null);
  const [memoryDraft, setMemoryDraft] = useState("");
  const [messages, setMessages] = useState<MessageDto[]>([]);
  const [live, setLive] = useState("");
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const homeDirRef = useRef<string | null>(null);
  const taskKeyRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const reload = useCallback(async () => {
    const p = await botzGetBot(agent.id);
    setProfile(p);
    setMemoryDraft(p.memory);
    homeDirRef.current ??= await botzHomeDir();
    if (p.home_session_id) {
      try {
        setMessages(await getMessages(p.home_session_id, homeDirRef.current));
      } catch {
        setMessages([]);
      }
    }
  }, [agent.id]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const sessionId = profile?.home_session_id ?? null;
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onChatEvent((env) => {
      if (!sessionId || env.sessionId !== sessionId || env.channel !== "agent_event") return;
      const evt = env.payload as AgentEvent;
      let chunk = "";
      if (evt.type === "text_delta") chunk = evt.delta;
      else if (evt.type === "tool_start") chunk = `\n▸ ${evt.name}\n`;
      else if (evt.type === "done") {
        setLive("");
        void reload();
      }
      if (chunk) setLive((prev) => (prev + chunk).slice(-LIVE_LIMIT));
    }).then((fn) => (disposed ? fn() : (unlisten = fn)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [sessionId, reload]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, live]);

  const save = useCallback(async (next: BotProfile) => {
    setProfile(await botzSaveBot(next));
  }, []);

  const send = useCallback(
    async (text: string, routine?: BotRoutine) => {
      if (!text.trim() || busy) return;
      setBusy(true);
      setError(null);
      setInput("");
      const key = crypto.randomUUID();
      taskKeyRef.current = key;
      setMessages((prev) => [...prev, { id: `local-${key}`, role: "user", content: routine ? `⏰ ${routine.name}\n${text}` : text }]);
      const res = await sendToBot({ agentId: agent.id, text, routine, preferZh: i18n.language.startsWith("zh"), taskKey: key });
      if (res.error === "budget") setError(t("botz.overBudget"));
      else if (res.error === "busy") setError(t("botz.botBusy"));
      else if (res.error) setError(res.error);
      setBusy(false);
      setLive("");
      await reload();
    },
    [agent.id, busy, i18n.language, reload, t],
  );

  const today = localDate(new Date());
  const updateRoutine = (id: string, patch: Partial<BotRoutine>) =>
    profile && void save({ ...profile, routines: profile.routines.map((r) => (r.id === id ? { ...r, ...patch } : r)) });

  return (
    <div className="botz-home">
      <div className="botz-home-main">
        <div className="botz-home-head">
          <button type="button" onClick={onBack} title={t("botz.viewBots")}>
            ←
          </button>
          <Avatar seed={agent.koi_id ?? agent.id} color={agent.color} icon={agent.icon} activity={busy ? "thinking" : activity} />
          <div>
            <div className="botz-bot-name">{agent.name}</div>
            <div className="botz-bot-role">{agent.role || agent.description}</div>
          </div>
        </div>
        <div className="botz-transcript" ref={scrollRef}>
          {messages.length === 0 && !live && <div className="botz-empty">{t("botz.homeEmpty", { name: agent.name })}</div>}
          {messages.map((m) => (
            <div key={m.id} className={`botz-msg ${m.role}`}>
              {m.content.trim() && <Markdown content={m.content.replace(/<memory>[\s\S]*?<\/memory>/gi, "").trim()} compact />}
              {m.tools && m.tools.length > 0 && (
                <div className="botz-msg-tools">
                  {m.tools.map((tl) => (
                    <span key={tl.id} className={tl.is_error ? "err" : ""}>
                      ▸ {tl.name}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}
          {live && <pre className="botz-live">{live}</pre>}
        </div>
        {error && (
          <div className="botz-error" onClick={() => setError(null)}>
            {error}
          </div>
        )}
        <div className="botz-composer">
          <textarea
            value={input}
            placeholder={t("botz.homePlaceholder", { name: agent.name })}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void send(input);
              }
            }}
          />
          {busy ? (
            <button type="button" onClick={() => void chatCancel(taskKeyRef.current)}>
              {t("botz.stop")}
            </button>
          ) : (
            <button type="button" disabled={!input.trim()} onClick={() => void send(input)}>
              {t("botz.send")}
            </button>
          )}
        </div>
      </div>

      {profile && (
        <aside className="botz-home-side">
          <section>
            <h4>{t("botz.memory")}</h4>
            <textarea className="botz-memory" value={memoryDraft} onChange={(e) => setMemoryDraft(e.target.value)} placeholder={t("botz.memoryPlaceholder")} />
            <button type="button" disabled={memoryDraft === profile.memory} onClick={() => void save({ ...profile, memory: memoryDraft })}>
              {t("botz.save")}
            </button>
          </section>

          <section>
            <h4>{t("botz.budget")}</h4>
            <div className="botz-budget">
              <input
                type="number"
                min={0}
                step={10000}
                value={profile.daily_token_budget}
                onChange={(e) => void save({ ...profile, daily_token_budget: Math.max(0, Number(e.target.value) || 0) })}
              />
              <span>
                {t("botz.usedToday", { used: usageToday(profile, today).toLocaleString() })}
                {profile.daily_token_budget > 0 &&
                  ` · ${Math.min(100, Math.round((usageToday(profile, today) / profile.daily_token_budget) * 100))}%`}
              </span>
            </div>
            <div className="botz-budget">
              <input
                type="number"
                min={0}
                step={100000}
                value={profile.monthly_token_budget ?? 0}
                onChange={(e) => void save({ ...profile, monthly_token_budget: Math.max(0, Number(e.target.value) || 0) })}
              />
              <span>
                {t("botz.monthlyBudget", { defaultValue: "monthly tokens" })}: {usageMonth(profile, today).toLocaleString()}
              </span>
            </div>
            {budgetLevel(profile, today) !== "ok" && (
              <div className="botz-error">
                {budgetLevel(profile, today) === "paused"
                  ? t("botz.budgetPaused", { defaultValue: "Budget reached: this bot is paused and will not start new turns." })
                  : t("botz.budgetWarn", { defaultValue: "Over 80% of the token budget is used." })}
              </div>
            )}
          </section>

          <section>
            <h4>
              {t("botz.routines")}
              <button type="button" onClick={() => void save({ ...profile, routines: [...profile.routines, newRoutine()] })}>
                ＋
              </button>
            </h4>
            {profile.routines.length === 0 && <div className="botz-empty">{t("botz.noRoutines")}</div>}
            {profile.routines.map((r) => (
              <div key={r.id} className="botz-routine">
                <div className="botz-routine-row">
                  <input type="checkbox" checked={r.enabled} onChange={(e) => updateRoutine(r.id, { enabled: e.target.checked })} />
                  <input
                    className="grow"
                    defaultValue={r.name}
                    placeholder={t("botz.routineName")}
                    onBlur={(e) => e.target.value !== r.name && updateRoutine(r.id, { name: e.target.value })}
                  />
                  <select value={r.kind} onChange={(e) => updateRoutine(r.id, { kind: e.target.value as BotRoutine["kind"] })}>
                    <option value="daily">{t("botz.daily")}</option>
                    <option value="interval">{t("botz.interval")}</option>
                  </select>
                  {r.kind === "daily" ? (
                    <input type="time" value={r.time ?? "09:00"} onChange={(e) => updateRoutine(r.id, { time: e.target.value })} />
                  ) : (
                    <input
                      type="number"
                      min={1}
                      value={r.minutes ?? 60}
                      title={t("botz.minutes")}
                      onChange={(e) => updateRoutine(r.id, { minutes: Math.max(1, Number(e.target.value) || 60) })}
                    />
                  )}
                </div>
                <textarea
                  defaultValue={r.prompt}
                  placeholder={t("botz.routinePrompt")}
                  onBlur={(e) => e.target.value !== r.prompt && updateRoutine(r.id, { prompt: e.target.value })}
                />
                <div className="botz-routine-row">
                  <span className="botz-empty">
                    {r.last_run_at ? t("botz.lastRun", { at: new Date(r.last_run_at).toLocaleString() }) : t("botz.neverRun")}
                  </span>
                  <button type="button" disabled={busy || !r.prompt.trim()} onClick={() => void send(r.prompt, r)}>
                    {t("botz.runNow")}
                  </button>
                  <button
                    type="button"
                    onClick={() => void save({ ...profile, routines: profile.routines.filter((x) => x.id !== r.id) })}
                  >
                    {t("botz.remove")}
                  </button>
                </div>
              </div>
            ))}
          </section>
        </aside>
      )}
    </div>
  );
}
