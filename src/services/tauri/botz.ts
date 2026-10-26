/**
 * BotZ room registry IPC. A room = name + working directory + its own members;
 * its pool and sessions live in that directory's project DB.
 */
import { invoke } from "@tauri-apps/api/core";
import type { KoiTodo } from "./pool";

export interface BotzRoom {
  id: string;
  name: string;
  dir: string;
  team_id: string;
  pool_id?: string | null;
  coordinator_session_id?: string | null;
  created_at?: string;
  /** Member agent ids, owned by the room. */
  members?: string[];
  org_spec?: string;
  /** Max member turns at once in this room; 0 or unset = global default. */
  max_concurrency?: number;
}

export interface RoomSync {
  room: BotzRoom;
  pool_id: string;
  member_koi_ids: string[];
  /** Members asked to leave that still own active todos. */
  kept_busy: string[];
}

/** Ensure the room's pool exists and matches its member list. */
export function botzOpenRoom(id: string): Promise<RoomSync> {
  return invoke<RoomSync>("botz_open_room", { id });
}

export function botzSetRoomMembers(id: string, members: string[], orgSpec?: string): Promise<RoomSync> {
  return invoke<RoomSync>("botz_set_room_members", { id, members, orgSpec: orgSpec ?? null });
}

export function botzListRooms(): Promise<BotzRoom[]> {
  return invoke<BotzRoom[]>("botz_list_rooms");
}

export function botzSaveRoom(room: BotzRoom): Promise<BotzRoom> {
  return invoke<BotzRoom>("botz_save_room", {
    room: { pool_id: null, coordinator_session_id: null, created_at: "", members: [], org_spec: "", ...room },
  });
}

export function botzDeleteRoom(id: string): Promise<void> {
  return invoke<void>("botz_delete_room", { id });
}

// ── Room board ──────────────────────────────────────────────────────────

/** All todos of a pool, including review / done. */
export function botzRoomTodos(projectDir: string, poolId: string): Promise<KoiTodo[]> {
  return invoke<KoiTodo[]>("botz_room_todos", { projectDir, poolId });
}

export type BoardStatus = "todo" | "blocked" | "needs_review" | "done" | "cancelled";

export function botzSetTodoStatus(projectDir: string, todoId: string, status: BoardStatus): Promise<void> {
  return invoke<void>("botz_set_todo_status", { projectDir, todoId, status });
}

/** Give one member work directly; patrol starts it when the member is free. */
export function botzAssignMember(projectDir: string, poolId: string, koiId: string, text: string): Promise<KoiTodo> {
  return invoke<KoiTodo>("botz_assign_member", { projectDir, poolId, koiId, text });
}

export interface TodoDiff {
  branch: string | null;
  base: string;
  stat: string;
  diff: string;
  truncated: boolean;
}

export function botzTodoDiff(projectDir: string, todoId: string): Promise<TodoDiff> {
  return invoke<TodoDiff>("botz_todo_diff", { projectDir, todoId });
}

export interface ReviewOutcome {
  status: string;
  merged: boolean;
  message: string;
}

/** Push the todo's branch and open a PR with `gh`; resolves to the PR URL. */
export function botzOpenPr(projectDir: string, todoId: string): Promise<string> {
  return invoke<string>("botz_open_pr", { projectDir, todoId });
}

export function botzReviewTodo(
  projectDir: string,
  todoId: string,
  approve: boolean,
  feedback?: string | null,
): Promise<ReviewOutcome> {
  return invoke<ReviewOutcome>("botz_review_todo", { projectDir, todoId, approve, feedback: feedback ?? null });
}

// ── Bot profiles ────────────────────────────────────────────────────────

export interface BotRoutine {
  id: string;
  name: string;
  prompt: string;
  kind: "interval" | "daily";
  minutes?: number | null;
  time?: string | null;
  enabled: boolean;
  last_run_at?: string | null;
}

export interface BotProfile {
  id: string;
  memory: string;
  routines: BotRoutine[];
  daily_token_budget: number;
  usage_date: string;
  usage_tokens: number;
  monthly_token_budget?: number;
  month_key?: string;
  month_tokens?: number;
  home_session_id?: string | null;
}

export function botzGetBot(id: string): Promise<BotProfile> {
  return invoke<BotProfile>("botz_get_bot", { id });
}

export function botzListBots(): Promise<BotProfile[]> {
  return invoke<BotProfile[]>("botz_list_bots");
}

export function botzSaveBot(profile: BotProfile): Promise<BotProfile> {
  return invoke<BotProfile>("botz_save_bot", { profile });
}

export function botzHomeDir(): Promise<string> {
  return invoke<string>("botz_home_dir");
}
