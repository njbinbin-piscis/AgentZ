import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { open as openDialog, ask } from "@tauri-apps/plugin-dialog";
import { listAgents, type AgentInfo } from "../../services/tauri/agents";
import { listTeams, type TeamInfo } from "../../services/tauri/teams";
import {
  botzDeleteRoom,
  botzListRooms,
  botzSaveRoom,
  botzSetRoomMembers,
  type BotzRoom,
} from "../../services/tauri/botz";
import Avatar from "./Avatar";
import BotPicker from "./BotPicker";
import RoomView from "./RoomView";
import BotHome from "./BotHomePage";
import type { Activity } from "./activity";
import "./botz.css";

export interface BotZWorkspaceProps {
  /** Current project, offered as the default directory for a new room. */
  projectDir: string | null;
}

type View = "bots" | "rooms";

const ACTIVITY_RANK: Activity[] = ["waiting", "error", "typing", "running", "reading", "thinking", "idle"];

function NewRoomForm({
  agents,
  teams,
  defaultDir,
  onCreate,
  onCancel,
}: {
  agents: AgentInfo[];
  teams: TeamInfo[];
  defaultDir: string | null;
  onCreate: (r: BotzRoom) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const swarm = teams.filter((tm) => tm.mode !== "workflow");
  const [name, setName] = useState("");
  const [dir, setDir] = useState(defaultDir ?? "");
  const [members, setMembers] = useState<string[]>([]);
  const [orgSpec, setOrgSpec] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const known = new Set(agents.map((a) => a.id));
  return (
    <div className="botz-newroom">
      <label>
        {t("botz.roomName")}
        <input value={name} autoFocus onChange={(e) => setName(e.target.value)} placeholder={t("botz.roomNamePlaceholder")} />
      </label>
      <label>
        {t("botz.roomDir")}
        <span className="botz-newroom-dir">
          <input value={dir} onChange={(e) => setDir(e.target.value)} />
          <button
            type="button"
            onClick={async () => {
              const picked = await openDialog({ directory: true, multiple: false, defaultPath: dir || undefined });
              if (typeof picked === "string") setDir(picked);
            }}
          >
            …
          </button>
        </span>
      </label>
      <div className="botz-field-label">
        <span>{t("botz.roomMembers", { count: members.length })}</span>
        {swarm.length > 0 && (
          <select
            value=""
            onChange={(e) => {
              const tm = swarm.find((x) => x.id === e.target.value);
              if (!tm) return;
              setMembers(tm.members.filter((m) => known.has(m)));
              if (!orgSpec.trim()) setOrgSpec(tm.org_spec ?? "");
              if (!name.trim()) setName(tm.name);
            }}
          >
            <option value="">{t("botz.fromTemplate")}</option>
            {swarm.map((tm) => (
              <option key={tm.id} value={tm.id}>
                {tm.name} ({tm.members.length})
              </option>
            ))}
          </select>
        )}
      </div>
      <BotPicker agents={agents} selected={members} onChange={setMembers} />
      <label>
        {t("botz.roomOrgSpec")}
        <textarea rows={3} value={orgSpec} onChange={(e) => setOrgSpec(e.target.value)} placeholder={t("botz.roomOrgSpecPlaceholder")} />
      </label>
      {err && <div className="botz-error">{err}</div>}
      <div className="botz-newroom-actions">
        <button type="button" onClick={onCancel}>
          {t("common.cancel")}
        </button>
        <button
          type="button"
          className="primary"
          disabled={!name.trim() || !dir.trim() || members.length === 0}
          onClick={async () => {
            try {
              await onCreate({ id: "", name, dir, team_id: "", members, org_spec: orgSpec });
            } catch (e) {
              setErr(String(e));
            }
          }}
        >
          {t("botz.createRoom")}
        </button>
      </div>
    </div>
  );
}

export default function BotZWorkspace({ projectDir }: BotZWorkspaceProps) {
  const { t } = useTranslation();
  const [view, setView] = useState<View>("rooms");
  const [rooms, setRooms] = useState<BotzRoom[]>([]);
  const [teams, setTeams] = useState<TeamInfo[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [activeRoom, setActiveRoom] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [roomActivity, setRoomActivity] = useState<Record<string, Record<string, Activity>>>({});
  const [focus, setFocus] = useState<{ roomId: string; koiId: string; nonce: number } | null>(null);
  const [openBotId, setOpenBotId] = useState<string | null>(null);
  const openBot = agents.find((a) => a.id === openBotId) ?? null;

  useEffect(() => {
    void botzListRooms().then((rs) => {
      setRooms(rs);
      setActiveRoom((cur) => cur ?? rs[0]?.id ?? null);
    });
    void listTeams().then(setTeams).catch(() => {});
    void listAgents().then(setAgents).catch(() => {});
  }, []);

  const saveRoom = useCallback(async (room: BotzRoom) => {
    const saved = await botzSaveRoom(room);
    setRooms((prev) => (prev.some((r) => r.id === saved.id) ? prev.map((r) => (r.id === saved.id ? saved : r)) : [...prev, saved]));
    return saved;
  }, []);

  const removeRoom = useCallback(
    async (room: BotzRoom) => {
      const ok = await ask(t("botz.deleteRoomConfirm", { name: room.name }), { kind: "warning" });
      if (!ok) return;
      await botzDeleteRoom(room.id);
      setRooms((prev) => prev.filter((r) => r.id !== room.id));
      setActiveRoom((cur) => (cur === room.id ? null : cur));
    },
    [t],
  );

  const onActivity = useCallback((roomId: string, byKoi: Record<string, Activity>) => {
    setRoomActivity((prev) => (prev[roomId] === byKoi ? prev : { ...prev, [roomId]: byKoi }));
  }, []);

  const teamById = useMemo(() => new Map(teams.map((tm) => [tm.id, tm])), [teams]);

  /** Legacy rooms list their members only on the template until first open. */
  const roomMembers = useCallback(
    (r: BotzRoom) => (r.members && r.members.length > 0 ? r.members : teamById.get(r.team_id)?.members ?? []),
    [teamById],
  );

  const botRooms = useCallback(
    (agent: AgentInfo) => rooms.filter((r) => roomMembers(r).includes(agent.id)),
    [rooms, roomMembers],
  );

  const syncedRoom = useCallback((room: BotzRoom) => {
    setRooms((prev) => prev.map((r) => (r.id === room.id ? room : r)));
  }, []);

  const [joinError, setJoinError] = useState<string | null>(null);
  const joinRoom = useCallback(
    async (agent: AgentInfo, room: BotzRoom) => {
      try {
        const synced = await botzSetRoomMembers(room.id, [...roomMembers(room), agent.id]);
        syncedRoom(synced.room);
        setJoinError(null);
      } catch (e) {
        setJoinError(String(e));
      }
    },
    [roomMembers, syncedRoom],
  );

  const botActivity = useCallback(
    (agent: AgentInfo): Activity => {
      const keys = [agent.koi_id, agent.id].filter(Boolean) as string[];
      let best: Activity = "idle";
      for (const byKoi of Object.values(roomActivity)) {
        for (const k of keys) {
          const a = byKoi[k];
          if (a && ACTIVITY_RANK.indexOf(a) < ACTIVITY_RANK.indexOf(best)) best = a;
        }
      }
      return best;
    },
    [roomActivity],
  );

  return (
    <div className="botz">
      <nav className="botz-nav">
        <button type="button" className={view === "bots" ? "active" : ""} onClick={() => setView("bots")}>
          🤖 {t("botz.viewBots")}
        </button>
        <button type="button" className={view === "rooms" ? "active" : ""} onClick={() => setView("rooms")}>
          🏢 {t("botz.viewRooms")}
        </button>
        {view === "rooms" && (
          <div className="botz-room-tabs">
            {rooms.map((r) => {
              const busy = Object.values(roomActivity[r.id] ?? {}).filter((a) => a !== "idle").length;
              const waiting = Object.values(roomActivity[r.id] ?? {}).filter((a) => a === "waiting").length;
              return (
                <div
                  key={r.id}
                  className={`botz-room-tab ${r.id === activeRoom ? "active" : ""}`}
                  onClick={() => {
                    setActiveRoom(r.id);
                    setCreating(false);
                  }}
                  title={r.dir}
                >
                  <span>{r.name}</span>
                  {busy > 0 && <span className="botz-badge">{busy}</span>}
                  {waiting > 0 && <span className="botz-badge warn">!</span>}
                  <button
                    type="button"
                    className="botz-room-tab-del"
                    title={t("botz.deleteRoom")}
                    onClick={(e) => {
                      e.stopPropagation();
                      void removeRoom(r);
                    }}
                  >
                    ×
                  </button>
                </div>
              );
            })}
            <button type="button" className="botz-room-add" onClick={() => setCreating(true)} title={t("botz.newRoom")}>
              ＋
            </button>
          </div>
        )}
      </nav>

      <div className="botz-body">
        {view === "bots" && openBot && (
          <BotHome
            agent={openBot}
            activity={botActivity(openBot)}
            onBack={() => setOpenBotId(null)}
          />
        )}
        {view === "bots" && !openBot && (
          <div className="botz-bots">
            {agents.length === 0 && <div className="botz-empty">{t("botz.noBots")}</div>}
            {joinError && (
              <div className="botz-error" onClick={() => setJoinError(null)}>
                {joinError}
              </div>
            )}
            {agents.map((a) => {
              const act = botActivity(a);
              const inRooms = botRooms(a);
              return (
                <div key={a.id} className="botz-bot-card" onClick={() => setOpenBotId(a.id)}>
                  <Avatar seed={a.koi_id ?? a.id} color={a.color} icon={a.icon} activity={act} />
                  <div className="botz-bot-info">
                    <div className="botz-bot-name">
                      {a.name}
                      <span className={`botz-dot act-${act}`} />
                      <span className="botz-bot-state">{t(`botz.act_${act}`)}</span>
                    </div>
                    <div className="botz-bot-role">{a.role || a.description}</div>
                    <div className="botz-bot-rooms">
                      {inRooms.length === 0 && <span className="botz-empty">{t("botz.botNoRooms")}</span>}
                      {inRooms.map((r) => (
                        <button
                          type="button"
                          key={r.id}
                          onClick={(e) => {
                            e.stopPropagation();
                            setView("rooms");
                            setActiveRoom(r.id);
                            setFocus({ roomId: r.id, koiId: a.koi_id ?? a.id, nonce: Date.now() });
                          }}
                        >
                          🏢 {r.name}
                        </button>
                      ))}
                      {rooms.some((r) => !inRooms.includes(r)) && (
                        <select
                          className="botz-join-room"
                          value=""
                          onClick={(e) => e.stopPropagation()}
                          onChange={(e) => {
                            const room = rooms.find((r) => r.id === e.target.value);
                            if (room) void joinRoom(a, room);
                          }}
                        >
                          <option value="">＋ {t("botz.joinRoom")}</option>
                          {rooms
                            .filter((r) => !inRooms.includes(r))
                            .map((r) => (
                              <option key={r.id} value={r.id}>
                                {r.name}
                              </option>
                            ))}
                        </select>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        <div className="botz-rooms" hidden={view !== "rooms"}>
          {creating || rooms.length === 0 ? (
            <div className="botz-rooms-empty">
              {rooms.length === 0 && !creating && <p>{t("botz.noRooms")}</p>}
              <NewRoomForm
                agents={agents}
                teams={teams}
                defaultDir={projectDir}
                onCancel={() => setCreating(false)}
                onCreate={async (r) => {
                  const saved = await saveRoom(r);
                  setActiveRoom(saved.id);
                  setCreating(false);
                }}
              />
            </div>
          ) : null}
          {rooms.map((r) => (
            <RoomView
              key={r.id}
              room={r}
              agents={agents}
              active={!creating && r.id === activeRoom}
              onRoomChange={(next) => void saveRoom(next)}
              onRoomSynced={syncedRoom}
              focusKoi={focus?.roomId === r.id ? { koiId: focus.koiId, nonce: focus.nonce } : null}
              onActivity={onActivity}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
