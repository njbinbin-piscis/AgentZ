import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { onChatEvent } from "../services/tauri/chat";
import {
  killBackgroundTask,
  listBackgroundTasks,
  type BackgroundTask,
  type BackgroundTaskEvent,
} from "../services/tauri/backgroundTasks";

/**
 * Live view of the project's registered background tasks.
 *
 * Reconciles from `background_list` on mount / project change / window
 * focus, then follows `background_task` events on the shared chat channel.
 * While anything is running it also polls every 5s so elapsed times and
 * tasks started by other panels stay fresh.
 */
export function useBackgroundTasks(projectDir: string | null) {
  const [tasks, setTasks] = useState<BackgroundTask[]>([]);
  const projectRef = useRef(projectDir);
  projectRef.current = projectDir;

  const refresh = useCallback(async () => {
    const dir = projectRef.current;
    if (!dir) {
      setTasks([]);
      return;
    }
    try {
      setTasks(await listBackgroundTasks(dir));
    } catch {
      // best-effort: keep the last known list
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh, projectDir]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onChatEvent((env) => {
      if (env.channel !== "background_task") return;
      const evt = env.payload as unknown as BackgroundTaskEvent;
      if (!evt?.task) return;
      setTasks((prev) => {
        const task = evt.task;
        const idx = prev.findIndex((x) => x.id === task.id);
        if (idx < 0) return [task, ...prev];
        const next = prev.slice();
        next[idx] = task;
        return next;
      });
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const running = useMemo(() => tasks.filter((x) => x.status === "running"), [tasks]);
  const runningSessionIds = useMemo(
    () => Array.from(new Set(running.map((x) => x.session_id))),
    [running],
  );

  useEffect(() => {
    if (running.length === 0) return;
    const timer = setInterval(() => void refresh(), 5000);
    return () => clearInterval(timer);
  }, [running.length, refresh]);

  const kill = useCallback(
    async (bgId: string) => {
      await killBackgroundTask(bgId).catch(() => false);
      // The terminal event updates the row; refresh covers a missed event.
      setTimeout(() => void refresh(), 600);
    },
    [refresh],
  );

  return { tasks, running, runningSessionIds, refresh, kill };
}
