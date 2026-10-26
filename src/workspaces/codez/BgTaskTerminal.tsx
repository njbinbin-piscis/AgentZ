import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  getBackgroundOutput,
  type BackgroundTask,
} from "../../services/tauri/backgroundTasks";
import { buildXtermTheme } from "./Terminal";

const POLL_MS = 600;

/**
 * Read-only terminal that follows one background task. The task's process is
 * owned by the kernel (piped stdio), so this replays its captured output and
 * keeps polling until the task finishes.
 */
export default function BgTaskTerminal({ taskId, visible }: { taskId: string; visible: boolean }) {
  const { t } = useTranslation();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [task, setTask] = useState<BackgroundTask | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!hostRef.current) return;
    const term = new XTerm({
      convertEol: true,
      disableStdin: true,
      cursorBlink: false,
      fontSize: 12,
      scrollback: 5000,
      theme: buildXtermTheme(),
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(hostRef.current);
    fitRef.current = fit;
    try {
      fit.fit();
    } catch {
      /* container not measurable yet */
    }

    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cursorOut = 0;
    let cursorErr = 0;
    setError(null);

    const tick = async () => {
      let finished = false;
      try {
        const res = await getBackgroundOutput(taskId, cursorOut, cursorErr);
        if (!alive) return;
        if (res.truncated) term.writeln(`\x1b[33m[${t("chat.bgTruncated")}]\x1b[0m`);
        if (res.stdout) term.write(res.stdout);
        if (res.stderr) term.write(res.stderr);
        cursorOut = res.next_cursor_out;
        cursorErr = res.next_cursor_err;
        setTask(res.task);
        finished = res.task.status !== "running";
        if (finished && cursorOut === 0 && cursorErr === 0) term.writeln(t("chat.bgNoOutput"));
      } catch (e) {
        if (!alive) return;
        setError(String(e));
        finished = true;
      }
      if (alive && !finished) timer = setTimeout(() => void tick(), POLL_MS);
    };
    void tick();

    const ro = new ResizeObserver(() => {
      try {
        fit.fit();
      } catch {
        /* ignore */
      }
    });
    ro.observe(hostRef.current);

    return () => {
      alive = false;
      clearTimeout(timer);
      ro.disconnect();
      term.dispose();
      fitRef.current = null;
    };
  }, [taskId, t]);

  useEffect(() => {
    if (visible) {
      try {
        fitRef.current?.fit();
      } catch {
        /* ignore */
      }
    }
  }, [visible]);

  return (
    <div className="agentz-bgterm" style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
      <div className="agentz-bgterm-head" style={{ padding: "2px 8px", fontSize: 12, opacity: 0.8 }}>
        {task ? `${task.label} · ${task.status}${task.exit_code != null ? ` (${task.exit_code})` : ""}` : taskId}
        {error ? ` · ${error}` : ""}
      </div>
      <div ref={hostRef} style={{ flex: 1, minHeight: 0 }} />
    </div>
  );
}