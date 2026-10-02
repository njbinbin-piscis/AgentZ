import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n";
import { extensionUiStore } from "./extensionUiStore";
import { browserNavigate } from "../services/tauri/browser";

interface ListeningPort {
  port: number;
}

interface Forward {
  remote_port: number;
  local_port: number;
}

const POLL_MS = 5000;
// System daemons (ssh, dns, ...) are never what the user wants to preview.
const MIN_PORT = 1024;

/**
 * VS Code-style automatic port forwarding: while connected to a remote,
 * ports that start listening after connect are forwarded and announced.
 * Returns the active forwards for the status bar.
 */
export function useRemotePortWatch(active: boolean): Forward[] {
  const [forwards, setForwards] = useState<Forward[]>([]);

  useEffect(() => {
    if (!active) {
      setForwards([]);
      return;
    }
    let cancelled = false;
    let known: Set<number> | null = null;

    const tick = async () => {
      try {
        const ports = await invoke<ListeningPort[]>("remote_ports_detect");
        const current = await invoke<Forward[]>("remote_forward_list");
        if (cancelled) return;
        const listening = new Set(ports.map((p) => p.port).filter((p) => p >= MIN_PORT));
        if (known) {
          const forwarded = new Set(current.map((f) => f.remote_port));
          for (const port of listening) {
            if (known.has(port) || forwarded.has(port)) continue;
            const fwd = await invoke<Forward>("remote_forward_start", { remotePort: port, localPort: null });
            current.push(fwd);
            const url = `http://localhost:${fwd.local_port}`;
            void extensionUiStore
              .pushMessage(1, i18n.t("remote.ports.autoForwarded", { remote: port, url }), undefined, [
                i18n.t("remote.ports.openPreview"),
              ])
              .then((choice) => {
                if (choice) void browserNavigate(url);
              });
          }
        }
        known = listening;
        setForwards(current);
      } catch {
        // Not connected yet / non-Linux remote: keep polling quietly.
      }
    };

    void tick();
    const timer = setInterval(() => void tick(), POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [active]);

  return forwards;
}
