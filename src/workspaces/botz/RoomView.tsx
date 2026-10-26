import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  chatCancel,
  chatSend,
  getMessages,
  onChatEvent,
  SESSION_SOURCE_WORKZ_TEAM,
  type AgentEvent,
  type MessageDto,
} from "../../services/tauri/chat";
import {
  onPoolEvent,
  poolMembers,
  poolMessages,
  type KoiTodo,
  type PoolMember,
  type PoolMessage,
} from "../../services/tauri/pool";
import type { AgentInfo } from "../../services/tauri/agents";
import {
  botzAssignMember,
  botzOpenRoom,
  botzRoomTodos,
  botzSetRoomMembers,
  botzSetTodoStatus,
  type BoardStatus,
  type BotzRoom,
} from "../../services/tauri/botz";
import Markdown from "../codez/Markdown";
import ToolTrace, { interleaveTools, upsertToolStep, truncateToolResultForUi, type ToolTraceItem } from "../../components/ToolTrace";
import Avatar from "./Avatar";
import BotPicker from "./BotPicker";
import ReviewPanel from "./ReviewPanel";
import { activityFromEvent, parentOf, unreadFor, deskGrid, koiIdFromSession, koiTaskSessionId, type Activity } from "./activity";

const COORDINATOR = "__coordinator__";
const BUBBLE_MS = 9000;

const BOARD_COLUMNS: { key: string; statuses: string[]; drop: BoardStatus | null }[] = [
  { key: "todo", statuses: ["todo", "backlog", "pending"], drop: "todo" },
  { key: "inProgress", statuses: ["in_progress"], drop: null },
  { key: "review", statuses: ["needs_review", "in_review", "review"], drop: "needs_review" },
  { key: "blocked", statuses: ["blocked"], drop: "blocked" },
  { key: "done", statuses: ["done", "completed", "cancelled"], drop: "done" },
];

export interface RoomViewProps {
  room: BotzRoom;
  /** All bots, for the member picker. */
  agents: AgentInfo[];
  /** Room state changed server-side (pool / members); update without saving. */
  onRoomSynced: (room: BotzRoom) => void;
  active: boolean;
  onRoomChange: (room: BotzRoom) => void;
  /** Desk to open when another view routes here (e.g. the bot roster). */
  focusKoi?: { koiId: string; nonce: number } | null;
  onActivity?: (roomId: string, byKoi: Record<string, Activity>) => void;
}

interface Bubble {
  text: string;
  at: number;
}

interface LiveTurn {
  text: string;
  tools: ToolTraceItem[];
}

function toTraceItems(tools: NonNullable<MessageDto["tools"]>): ToolTraceItem[] {
  return tools.map((tl) => ({
    id: tl.id,
    name: tl.name,
    input: tl.input,
    result: tl.result ?? undefined,
    status: tl.is_error ? "error" : "done",
    textOffset: tl.text_offset,
  }));
}

function Segments({ text, tools }: { text: string; tools: ToolTraceItem[] }) {
  return (
    <>
      {interleaveTools(text, tools).map((seg, i) => (
        <div key={i}>
          {seg.text.trim() && <Markdown content={seg.text} compact />}
          {seg.tools.length > 0 && <ToolTrace items={seg.tools} />}
        </div>
      ))}
    </>
  );
}

function SessionTranscript({ messages, live }: { messages: MessageDto[]; live?: LiveTurn }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, live]);
  return (
    <div className="botz-transcript" ref={ref}>
      {messages.map((m) => (
        <div key={m.id} className={`botz-msg ${m.role}`}>
          {m.role === "user" ? (
            <div className="botz-msg-user">{m.content}</div>
          ) : (
            <Segments text={m.content} tools={toTraceItems(m.tools ?? [])} />
          )}
        </div>
      ))}
      {live && (live.text || live.tools.length > 0) && (
        <div className="botz-msg assistant">
          <Segments text={live.text} tools={live.tools} />
        </div>
      )}
    </div>
  );
}

export default function RoomView({ room, agents, active, onRoomChange, onRoomSynced, focusKoi, onActivity }: RoomViewProps) {
  const { t, i18n } = useTranslation();
  const [poolId, setPoolId] = useState<string | null>(room.pool_id ?? null);
  const [members, setMembers] = useState<PoolMember[]>([]);
  const [todos, setTodos] = useState<KoiTodo[]>([]);
  const [feed, setFeed] = useState<PoolMessage[]>([]);
  const [activity, setActivity] = useState<Record<string, Activity>>({});
  const [live, setLive] = useState<Record<string, LiveTurn>>({});
  const [bubbles, setBubbles] = useState<Record<string, Bubble>>({});
  const [desk, setDesk] = useState<string | null>(null);
  // Mailbox: a member's inbox is the pool messages that @mention them; opening
  // the desk marks everything up to the newest message as read.
  const mailKey = `botz-mail-${room.id}`;
  const [lastRead, setLastRead] = useState<Record<string, number>>(() => {
    try {
      return JSON.parse(localStorage.getItem(mailKey) ?? "{}") as Record<string, number>;
    } catch {
      return {};
    }
  });
  const [todoSel, setTodoSel] = useState<string | null>(null);
  const [transcript, setTranscript] = useState<MessageDto[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showBoard, setShowBoard] = useState(true);
  const [reviewing, setReviewing] = useState<KoiTodo | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [memberInput, setMemberInput] = useState("");

  const moveTodo = useCallback(
    async (todoId: string, status: BoardStatus) => {
      try {
        await botzSetTodoStatus(room.dir, todoId, status);
      } catch (e) {
        setError(String(e));
      }
    },
    [room.dir],
  );
  const roomRef = useRef(room);
  roomRef.current = room;
  const taskKeyRef = useRef<string | null>(null);
  const seenMsgRef = useRef<number>(0);
  const pendingRef = useRef<{ sid: string; msg: MessageDto }[]>([]);

  // Ensure the room's pool exists; this also starts its patrol / heartbeat loops.
  useEffect(() => {
    let cancelled = false;
    botzOpenRoom(room.id)
      .then((synced) => {
        if (cancelled) return;
        setPoolId(synced.pool_id);
        onRoomSynced(synced.room);
      })
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [room.id, room.dir]);

  const [editingMembers, setEditingMembers] = useState(false);
  const [memberDraft, setMemberDraft] = useState<string[]>([]);
  const saveMembers = useCallback(async () => {
    try {
      const synced = await botzSetRoomMembers(room.id, memberDraft);
      setPoolId(synced.pool_id);
      onRoomSynced(synced.room);
      setEditingMembers(false);
      if (synced.kept_busy.length > 0) {
        setNotice(t("botz.membersKeptBusy", { names: synced.kept_busy.join(", ") }));
      }
    } catch (e) {
      setError(String(e));
    }
  }, [memberDraft, onRoomSynced, room.id, t]);

  const refresh = useCallback(async () => {
    if (!poolId) return;
    try {
      const [m, td, msg] = await Promise.all([
        poolMembers(room.dir, poolId),
        botzRoomTodos(room.dir, poolId),
        poolMessages(room.dir, poolId, 80, 0),
      ]);
      setMembers(m);
      setTodos(td);
      setFeed(msg);
    } catch {
      // next pool event retries
    }
  }, [room.dir, poolId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let timer: number | undefined;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onPoolEvent(() => {
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(() => void refresh(), 300);
    }).then((fn) => (disposed ? fn() : (unlisten = fn)));
    return () => {
      disposed = true;
      unlisten?.();
      if (timer) window.clearTimeout(timer);
    };
  }, [refresh]);

  // Speech bubbles for fresh pool messages.
  useEffect(() => {
    const newest = feed.reduce((mx, m) => Math.max(mx, m.id), 0);
    if (seenMsgRef.current === 0) {
      seenMsgRef.current = newest;
      return;
    }
    const fresh = feed.filter((m) => m.id > seenMsgRef.current && m.content.trim());
    seenMsgRef.current = Math.max(seenMsgRef.current, newest);
    if (fresh.length === 0) return;
    const now = Date.now();
    setBubbles((prev) => {
      const next = { ...prev };
      for (const m of fresh) {
        const who = m.sender_id === "piscis" ? COORDINATOR : m.sender_id;
        next[who] = { text: m.content.slice(0, 120), at: now };
      }
      return next;
    });
    const timer = window.setTimeout(() => {
      setBubbles((prev) => {
        const cut = Date.now() - BUBBLE_MS;
        return Object.fromEntries(Object.entries(prev).filter(([, b]) => b.at > cut));
      });
    }, BUBBLE_MS + 50);
    return () => window.clearTimeout(timer);
  }, [feed]);

  useEffect(() => {
    if (!desk || desk === COORDINATOR || feed.length === 0) return;
    const newest = feed.reduce((mx, m) => Math.max(mx, m.id), 0);
    if ((lastRead[desk] ?? 0) >= newest) return;
    const next = { ...lastRead, [desk]: newest };
    setLastRead(next);
    try {
      localStorage.setItem(mailKey, JSON.stringify(next));
    } catch {
      // storage unavailable
    }
  }, [desk, feed, lastRead, mailKey]);

  const todoPrefixes = useMemo(() => new Set(todos.map((td) => td.id.slice(0, 8))), [todos]);
  const koiIds = useMemo(() => members.map((m) => m.koi_id), [members]);
  const scopeRef = useRef({ todoPrefixes, koiIds });
  scopeRef.current = { todoPrefixes, koiIds };

  // Live activity + transcript tail from the shared chat event channel.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onChatEvent((env) => {
      if (env.channel !== "agent_event" || !env.sessionId) return;
      const sid = env.sessionId;
      let who: string | null = null;
      if (sid === roomRef.current.coordinator_session_id) {
        who = COORDINATOR;
      } else {
        const koi = koiIdFromSession(sid, scopeRef.current.koiIds);
        // Same agent may sit in several rooms; the todo id keeps events in their room.
        if (koi && scopeRef.current.todoPrefixes.has(sid.slice(-8))) who = koi;
      }
      if (!who) return;
      const evt = env.payload as AgentEvent;
      setActivity((prev) => {
        const next = activityFromEvent(evt, prev[who!] ?? "idle");
        return next && next !== prev[who!] ? { ...prev, [who!]: next } : prev;
      });
      if (evt.type !== "text_delta" && evt.type !== "tool_start" && evt.type !== "tool_end") return;
      setLive((prev) => {
        const cur = prev[sid] ?? { text: "", tools: [] };
        let next: LiveTurn = cur;
        if (evt.type === "text_delta") next = { ...cur, text: cur.text + evt.delta };
        else if (evt.type === "tool_start") {
          next = { ...cur, tools: upsertToolStep(cur.tools, evt, cur.text.length) };
        } else {
          next = {
            ...cur,
            tools: cur.tools.map((x) =>
              x.id === evt.id
                ? { ...x, status: evt.is_error ? "error" : "done", result: truncateToolResultForUi(evt.result) }
                : x,
            ),
          };
        }
        return { ...prev, [sid]: next };
      });
    }).then((fn) => (disposed ? fn() : (unlisten = fn)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Seed idle/working state from the pool's member status when no live event arrived yet.
  const deskActivity = useCallback(
    (m: PoolMember): Activity => {
      const a = activity[m.koi_id];
      if (a) return a;
      if (m.status === "busy" || m.status === "working" || m.status === "running") return "thinking";
      if (m.status === "error" || m.status === "failed") return "error";
      return "idle";
    },
    [activity],
  );

  useEffect(() => {
    onActivity?.(room.id, activity);
  }, [activity, onActivity, room.id]);

  const focusNonce = focusKoi?.nonce;
  const focusId = focusKoi?.koiId;
  useEffect(() => {
    if (focusNonce && focusId) setDesk(focusId);
  }, [focusNonce, focusId]);

  const deskTodos = useMemo(
    () => (desk && desk !== COORDINATOR ? todos.filter((td) => td.owner_id === desk) : []),
    [desk, todos],
  );

  // Default member selection: in-progress todo first, then the latest.
  useEffect(() => {
    if (!desk || desk === COORDINATOR) return;
    if (todoSel && deskTodos.some((td) => td.id === todoSel)) return;
    const pick = deskTodos.find((td) => td.status === "in_progress") ?? deskTodos[deskTodos.length - 1];
    setTodoSel(pick?.id ?? null);
  }, [desk, deskTodos, todoSel]);

  const openSessionId =
    desk === COORDINATOR
      ? (room.coordinator_session_id ?? null)
      : desk && todoSel
        ? koiTaskSessionId(desk, todoSel)
        : null;

  const loadTranscript = useCallback(async () => {
    if (!openSessionId) {
      setTranscript([]);
      return;
    }
    let fetched: MessageDto[] = [];
    try {
      fetched = await getMessages(openSessionId, room.dir);
    } catch {
      // keep pending only
    }
    const extra = pendingRef.current
      .filter((p) => p.sid === openSessionId && !fetched.some((m) => m.role === "user" && m.content === p.msg.content))
      .map((p) => p.msg);
    setTranscript([...fetched, ...extra]);
  }, [openSessionId, room.dir]);

  useEffect(() => {
    void loadTranscript();
  }, [loadTranscript]);

  // Reload a member transcript when its turn finishes.
  useEffect(() => {
    if (!desk || desk === COORDINATOR) return;
    if (activity[desk] === "idle") {
      void loadTranscript().then(() => {
        const sid = koiTaskSessionId(desk, todoSel ?? "");
        setLive((prev) => (prev[sid] ? { ...prev, [sid]: { text: "", tools: [] } } : prev));
      });
    }
  }, [activity, desk, loadTranscript, todoSel]);

  const sendToTeam = useCallback(async () => {
    const text = input.trim();
    if (!text || busy || !poolId) return;
    setBusy(true);
    setError(null);
    setInput("");
    const first = !roomRef.current.coordinator_session_id;
    const sessionId = roomRef.current.coordinator_session_id ?? crypto.randomUUID();
    if (first) onRoomChange({ ...roomRef.current, coordinator_session_id: sessionId });
    const taskKey = crypto.randomUUID();
    taskKeyRef.current = taskKey;
    setDesk(COORDINATOR);
    const localMsg: MessageDto = { id: `local-${taskKey}`, role: "user", content: text };
    pendingRef.current.push({ sid: sessionId, msg: localMsg });
    setTranscript((prev) => [...prev, localMsg]);
    let prompt = text;
    if (first) {
      const org = room.org_spec?.trim();
      const orgSection = org
        ? `\n\nYour team operates under this organization contract (org_spec); every member assistant receives it too, so hold yourself and them to it:\n${org}`
        : "";
      prompt =
        `You are the coordinator of swarm team "${room.name}" (collaboration pool id: ${poolId}) ` +
        `working in room "${room.name}" at ${room.dir}. Use the pool_org and pool_chat tools to break this ` +
        `down into todos, assign them to member assistants, and integrate their results.${orgSection}\n\nTask:\n${text}`;
    }
    try {
      await chatSend({
        prompt,
        displayPrompt: text,
        sessionId,
        projectDir: room.dir,
        chatMode: "agent",
        clearPlan: false,
        taskKey,
        sessionSource: SESSION_SOURCE_WORKZ_TEAM,
        teamId: room.team_id,
        poolId,
        preferZh: i18n.language.startsWith("zh"),
      });
    } catch (e) {
      setError(String(e));
    } finally {
      taskKeyRef.current = null;
      setBusy(false);
      setActivity((prev) => ({ ...prev, [COORDINATOR]: "idle" }));
      try {
        const fetched = await getMessages(sessionId, room.dir);
        if (fetched.some((m) => m.role === "user" && m.content === text)) {
          pendingRef.current = pendingRef.current.filter((p) => p.msg.id !== localMsg.id);
        }
        setTranscript(fetched);
      } catch {
        // keep optimistic transcript
      }
      setLive((prev) => ({ ...prev, [sessionId]: { text: "", tools: [] } }));
      void refresh();
    }
  }, [busy, i18n.language, input, onRoomChange, poolId, refresh, room.dir, room.name, room.org_spec, room.team_id]);

  const assignToMember = useCallback(async () => {
    const text = memberInput.trim();
    if (!text || !poolId || !desk || desk === COORDINATOR) return;
    setMemberInput("");
    try {
      const td = await botzAssignMember(room.dir, poolId, desk, text);
      setTodoSel(td.id);
      void refresh();
    } catch (e) {
      setError(String(e));
      setMemberInput(text);
    }
  }, [desk, memberInput, poolId, refresh, room.dir]);

  const grid = deskGrid(members.length);
  const coordActivity = activity[COORDINATOR] ?? (busy ? "thinking" : "idle");
  const memberById = useMemo(() => new Map(members.map((m) => [m.koi_id, m])), [members]);
  const deskMember = desk && desk !== COORDINATOR ? memberById.get(desk) : undefined;

  return (
    <div className="botz-room" hidden={!active}>
      <div className="botz-room-main">
        <div className="botz-room-toolbar">
          <span className="botz-room-dir" title={room.dir}>
            📁 {room.dir}
          </span>
          <span className="botz-room-members-anchor">
            <button
              type="button"
              className={editingMembers ? "active" : ""}
              onClick={() => {
                setMemberDraft(room.members ?? []);
                setEditingMembers((v) => !v);
              }}
            >
              👥 {t("botz.membersCount", { count: room.members?.length ?? members.length })}
            </button>
            {editingMembers && (
              <div className="botz-member-editor">
                <BotPicker agents={agents} selected={memberDraft} onChange={setMemberDraft} />
                <div className="botz-newroom-actions">
                  <button type="button" onClick={() => setEditingMembers(false)}>
                    {t("common.cancel")}
                  </button>
                  <button type="button" className="primary" onClick={() => void saveMembers()}>
                    {t("common.save")}
                  </button>
                </div>
              </div>
            )}
          </span>
          <label className="botz-concurrency" title={t("botz.concurrencyHint", { defaultValue: "Max members working at once" })}>
            ⚡
            <select
              value={room.max_concurrency ?? 0}
              onChange={(e) => onRoomChange({ ...room, max_concurrency: Number(e.target.value) })}
            >
              <option value={0}>{t("botz.concurrencyDefault", { defaultValue: "auto" })}</option>
              {[1, 2, 3, 4, 5, 6, 8].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className={showBoard ? "active" : ""} onClick={() => setShowBoard((v) => !v)}>
            {t("botz.board")}
          </button>
        </div>
        {error && (
          <div className="botz-error" onClick={() => setError(null)}>
            {error}
          </div>
        )}
        <div className="botz-floor">
          <div
            className={`botz-desk boss ${desk === COORDINATOR ? "selected" : ""}`}
            onClick={() => setDesk(COORDINATOR)}
            title={t("botz.coordinator")}
          >
            {bubbles[COORDINATOR] && <div className="botz-bubble">{bubbles[COORDINATOR].text}</div>}
            {coordActivity === "waiting" && <div className="botz-alert">!</div>}
            <Avatar seed={`coord-${room.id}`} color="#2d6cdf" icon="⭐" activity={coordActivity} boss />
            <div className="botz-nameplate">
              {t("botz.coordinator")}
              <span className={`botz-dot act-${coordActivity}`} />
            </div>
          </div>
          <div className="botz-desks" style={{ gridTemplateColumns: `repeat(${grid.cols}, minmax(130px, 1fr))` }}>
            {members.length === 0 && <div className="botz-empty">{t("botz.noMembers")}</div>}
            {members.map((m) => {
              const act = deskActivity(m);
              return (
                <div
                  key={m.koi_id}
                  className={`botz-desk ${desk === m.koi_id ? "selected" : ""}`}
                  onClick={() => setDesk(m.koi_id)}
                  title={m.role}
                >
                  {bubbles[m.koi_id] && <div className="botz-bubble">{bubbles[m.koi_id].text}</div>}
                  {act === "waiting" && <div className="botz-alert">!</div>}
                  {desk !== m.koi_id && unreadFor(feed, m.koi_id, m.name, lastRead[m.koi_id] ?? 0) > 0 && (
                    <div className="botz-mail" title={t("botz.unreadMail", { defaultValue: "Unread messages" })}>
                      ✉ {unreadFor(feed, m.koi_id, m.name, lastRead[m.koi_id] ?? 0)}
                    </div>
                  )}
                  <Avatar seed={m.koi_id} color={m.color} icon={m.icon} activity={act} />
                  <div className="botz-nameplate">
                    {m.name}
                    <span className={`botz-dot act-${act}`} />
                  </div>
                  <div className="botz-desk-task">
                    {todos.find((td) => td.owner_id === m.koi_id && td.status === "in_progress")?.title ?? t(`botz.act_${act}`)}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
        {showBoard && (
          <div className="botz-board">
            {BOARD_COLUMNS.map((col) => {
              const items = todos.filter((td) => col.statuses.includes(td.status));
              return (
                <div
                  key={col.key}
                  className={`botz-board-col ${dragId && col.drop ? "droppable" : ""}`}
                  onDragOver={(e) => {
                    if (col.drop && dragId) e.preventDefault();
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    const id = dragId;
                    setDragId(null);
                    if (col.drop && id && todos.find((x) => x.id === id)?.status !== col.drop) {
                      void moveTodo(id, col.drop);
                    }
                  }}
                >
                  <div className="botz-board-head">
                    {t(`botz.col_${col.key}`)} <span>{items.length}</span>
                  </div>
                  {items.map((td) => {
                    const owner = memberById.get(td.owner_id);
                    return (
                      <div
                        key={td.id}
                        className={`botz-card ${td.status}`}
                        draggable={td.status !== "in_progress"}
                        onDragStart={(e) => {
                          e.dataTransfer.effectAllowed = "move";
                          setDragId(td.id);
                        }}
                        onDragEnd={() => setDragId(null)}
                        title={td.description}
                        onClick={() => {
                          if (td.status === "needs_review") {
                            setReviewing(td);
                          } else if (owner) {
                            setDesk(owner.koi_id);
                            setTodoSel(td.id);
                          }
                        }}
                      >
                        <span className="botz-card-owner" style={{ background: owner?.color || "#7c6af7" }}>
                          {owner?.icon || "🐟"}
                        </span>
                        <span className="botz-card-title">
                          {parentOf(td, todos) && (
                            <span className="botz-card-parent" title={parentOf(td, todos)?.title}>
                              ↳{" "}
                            </span>
                          )}
                          {td.title}
                        </span>
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {desk && (
        <aside className="botz-drawer">
          <div className="botz-drawer-head">
            <strong>{desk === COORDINATOR ? t("botz.coordinator") : (deskMember?.name ?? desk)}</strong>
            {deskMember?.role && <span className="botz-drawer-role">{deskMember.role}</span>}
            <button type="button" onClick={() => setDesk(null)} title={t("common.close")}>
              ✕
            </button>
          </div>
          {desk !== COORDINATOR && (
            <div className="botz-drawer-todos">
              {deskTodos.length === 0 && <span className="botz-empty">{t("botz.noAssignments")}</span>}
              {deskTodos.map((td) => (
                <button
                  type="button"
                  key={td.id}
                  className={`${td.status} ${td.id === todoSel ? "active" : ""}`}
                  onClick={() => setTodoSel(td.id)}
                  title={td.description}
                >
                  {td.title}
                </button>
              ))}
            </div>
          )}
          <SessionTranscript messages={transcript} live={openSessionId ? live[openSessionId] : undefined} />
          {desk === COORDINATOR ? (
            <div className="botz-composer">
              <textarea
                value={input}
                placeholder={t("botz.composerPlaceholder")}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    void sendToTeam();
                  }
                }}
              />
              {busy ? (
                <button type="button" onClick={() => void chatCancel(taskKeyRef.current)}>
                  {t("botz.stop")}
                </button>
              ) : (
                <button type="button" disabled={!input.trim() || !poolId} onClick={() => void sendToTeam()}>
                  {t("botz.send")}
                </button>
              )}
            </div>
          ) : (
            <div className="botz-composer">
              <textarea
                value={memberInput}
                placeholder={t("botz.memberPlaceholder", { name: deskMember?.name ?? desk })}
                onChange={(e) => setMemberInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    void assignToMember();
                  }
                }}
              />
              <button type="button" disabled={!memberInput.trim() || !poolId} onClick={() => void assignToMember()}>
                {t("botz.assign")}
              </button>
            </div>
          )}
        </aside>
      )}

      {reviewing && (
        <ReviewPanel
          projectDir={room.dir}
          todo={reviewing}
          ownerName={memberById.get(reviewing.owner_id)?.name ?? reviewing.owner_id}
          onClose={() => setReviewing(null)}
          onDone={(message) => {
            setReviewing(null);
            setNotice(message);
            window.setTimeout(() => setNotice(null), 5000);
            void refresh();
          }}
        />
      )}
      {notice && <div className="botz-notice">{notice}</div>}
    </div>
  );
}
