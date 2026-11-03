import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import i18n from "../i18n";
import { extensionUiStore } from "./extensionUiStore";
import { browserNavigate } from "../services/tauri/browser";
import { getActiveChatSession } from "../services/activeChatSession";
import { isRemoteDir } from "./remoteTargets";
import { remoteLocalhostPort, urlOnForward } from "./remoteBrowserUrl";

export { remoteLocalhostPort, urlOnForward };

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
 * While a remote project is open, localhost in the built-in browser means the
 * remote machine. Forward that port and open the local end.
 */
export async function rewriteRemoteBrowserUrl(projectDir: string | null | undefined, url: string): Promise<string> {
  if (!isRemoteDir(projectDir)) return url;
  const remotePort = remoteLocalhostPort(url);
  if (remotePort === null) return url;
  const current = await invoke<Forward[]>("remote_forward_list");
  let fwd = current.find((item) => item.remote_port === remotePort);
  if (!fwd) {
    fwd = await invoke<Forward>("remote_forward_start", { remotePort, localPort: null });
  }
  return urlOnForward(url, fwd.local_port);
}

/**
 * Automatic port forwarding: while connected to a remote, listening ports
 * (already up, and ones that appear later) are forwarded. New ones are announced.
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
        const forwarded = new Set(current.map((f) => f.remote_port));
        for (const port of listening) {
          if (forwarded.has(port)) continue;
          const fwd = await invoke<Forward>("remote_forward_start", { remotePort: port, localPort: null });
          current.push(fwd);
          // Announce ports that appear after the first look, so a reconnect does not toast every daemon.
          if (known) {
            const url = `http://127.0.0.1:${fwd.local_port}`;
            void extensionUiStore
              .pushMessage(1, i18n.t("remote.ports.autoForwarded", { remote: port, url }), undefined, [
                i18n.t("remote.ports.openPreview"),
              ])
              .then((choice) => {
                if (choice) void browserNavigate(getActiveChatSession() ?? "", url);
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
