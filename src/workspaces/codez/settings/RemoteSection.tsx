import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  rememberTarget,
  toRemoteDir,
  isRemoteDir,
  describeRemoteDir,
  openProject,
  type RemoteTarget,
} from "../../../extensions/remoteTargets";

interface RemoteTargets {
  ssh_hosts: string[];
  wsl_distros: string[];
  containers: { id: string; name: string; image: string; status: string }[];
}

interface DevcontainerUp {
  container_id: string;
  remote_user: string | null;
  remote_workspace_folder: string;
  extensions: string[];
}

type Kind = Exclude<RemoteTarget["kind"], "local"> | "devcontainer";

interface ListeningPort {
  port: number;
  address: string;
}

interface Forward {
  remote_port: number;
  local_port: number;
  via: string;
}

function PortsPanel() {
  const { t } = useTranslation();
  const [ports, setPorts] = useState<ListeningPort[]>([]);
  const [forwards, setForwards] = useState<Forward[]>([]);
  const [manual, setManual] = useState("");
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setError(null);
    try {
      setForwards(await invoke<Forward[]>("remote_forward_list"));
      setPorts(await invoke<ListeningPort[]>("remote_ports_detect"));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const start = async (remotePort: number) => {
    setError(null);
    try {
      await invoke("remote_forward_start", { remotePort, localPort: null });
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };

  const stop = async (remotePort: number) => {
    await invoke("remote_forward_stop", { remotePort }).catch((e) => setError(String(e)));
    await reload();
  };

  const forwarded = new Set(forwards.map((f) => f.remote_port));
  const manualPort = Number(manual);

  return (
    <div style={{ marginTop: 12 }}>
      <h4>{t("remote.ports.title")}</h4>
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <button type="button" onClick={() => void reload()}>
          {t("remote.ports.detect")}
        </button>
        <input
          value={manual}
          placeholder={t("remote.ports.manual")}
          onChange={(e) => setManual(e.target.value.replace(/\D/g, ""))}
          style={{ width: 120 }}
        />
        <button
          type="button"
          disabled={!(manualPort > 0 && manualPort < 65536)}
          onClick={() => void start(manualPort)}
        >
          {t("remote.ports.forward")}
        </button>
      </div>
      {forwards.map((f) => (
        <div key={f.remote_port} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 4 }}>
          <span>
            {t("remote.ports.active", { remote: f.remote_port, via: f.via })}{" "}
            <a href={`http://localhost:${f.local_port}`} target="_blank" rel="noreferrer">
              localhost:{f.local_port}
            </a>
          </span>
          <button type="button" onClick={() => void stop(f.remote_port)}>
            {t("remote.ports.stop")}
          </button>
        </div>
      ))}
      {ports
        .filter((p) => !forwarded.has(p.port))
        .map((p) => (
          <div key={p.port} style={{ display: "flex", gap: 8, alignItems: "center", marginTop: 4 }}>
            <span>
              {p.port} ({p.address})
            </span>
            <button type="button" onClick={() => void start(p.port)}>
              {t("remote.ports.forward")}
            </button>
          </div>
        ))}
      {ports.length === 0 && forwards.length === 0 && (
        <p className="agentz-settings-hint">{t("remote.ports.none")}</p>
      )}
      {error && <div className="agentz-settings-error">{error}</div>}
    </div>
  );
}

export default function RemoteSection({ projectDir }: { projectDir: string }) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<RemoteTargets | null>(null);
  const [kind, setKind] = useState<Kind>("ssh");
  const [name, setName] = useState("");
  const [folder, setFolder] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState("");

  const setupKey = useCallback(async () => {
    setBusy(true);
    setError(null);
    setStatus(t("remote.key.installing"));
    try {
      await invoke<string>("remote_ssh_setup_key", { host: name.trim(), password });
      setPassword("");
      setStatus(t("remote.key.done"));
    } catch (e) {
      setStatus(null);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [name, password, t]);

  const refresh = useCallback(() => {
    invoke<RemoteTargets>("remote_list_targets")
      .then(setTargets)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(refresh, [refresh]);

  const connect = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      let target: RemoteTarget;
      let dir = folder.trim();
      if (kind === "devcontainer") {
        if (isRemoteDir(projectDir)) throw new Error(t("remote.devcontainerLocalOnly"));
        setStatus(t("remote.buildingContainer"));
        const up = await invoke<DevcontainerUp>("remote_devcontainer_up", { workspace: projectDir });
        target = { kind: "docker", container: up.container_id, user: up.remote_user };
        dir = dir || up.remote_workspace_folder;
      } else if (kind === "ssh") {
        target = { kind: "ssh", host: name.trim() };
      } else if (kind === "wsl") {
        target = { kind: "wsl", distro: name.trim() };
      } else {
        target = { kind: "docker", container: name.trim() };
      }
      setStatus(t("remote.probing"));
      const probe = await invoke<string>("remote_probe", { target });
      if (!dir) dir = probe.trim().split("\n").pop()?.trim() || "/";
      rememberTarget(target);
      setStatus(t("remote.starting"));
      openProject(toRemoteDir(target, dir));
    } catch (e) {
      setStatus(null);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [folder, kind, name, projectDir, t]);

  const suggestions =
    kind === "ssh"
      ? targets?.ssh_hosts ?? []
      : kind === "wsl"
        ? targets?.wsl_distros ?? []
        : kind === "docker"
          ? (targets?.containers ?? []).map((c) => c.name || c.id)
          : [];

  const needsName = kind !== "devcontainer";
  const current = describeRemoteDir(projectDir);

  return (
    <section className="agentz-settings-section">
      <h3>{t("remote.title")}</h3>
      <p className="agentz-settings-hint">{t("remote.hint")}</p>
      {current && <p className="agentz-settings-hint">{t("remote.current", { target: current })}</p>}
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
        <select value={kind} onChange={(e) => setKind(e.target.value as Kind)} disabled={busy}>
          <option value="ssh">SSH</option>
          <option value="devcontainer">{t("remote.devcontainer")}</option>
          <option value="docker">Docker</option>
          <option value="wsl">WSL</option>
        </select>
        {needsName && (
          <>
            <input
              list="agentz-remote-suggestions"
              value={name}
              placeholder={t(`remote.placeholder.${kind}`)}
              onChange={(e) => setName(e.target.value)}
              disabled={busy}
            />
            <datalist id="agentz-remote-suggestions">
              {suggestions.map((s) => (
                <option key={s} value={s} />
              ))}
            </datalist>
          </>
        )}
        <input
          value={folder}
          placeholder={t("remote.folderPlaceholder")}
          onChange={(e) => setFolder(e.target.value)}
          disabled={busy}
        />
        <button type="button" disabled={busy || (needsName && !name.trim())} onClick={() => void connect()}>
          {t("remote.connect")}
        </button>
        <button type="button" disabled={busy} onClick={refresh}>
          {t("remote.refresh")}
        </button>
      </div>
      {kind === "ssh" && (
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center", marginTop: 8 }}>
          <span className="agentz-settings-hint">{t("remote.key.hint")}</span>
          <input
            type="password"
            value={password}
            placeholder={t("remote.key.password")}
            onChange={(e) => setPassword(e.target.value)}
            disabled={busy}
          />
          <button type="button" disabled={busy || !name.trim() || !password} onClick={() => void setupKey()}>
            {t("remote.key.setup")}
          </button>
        </div>
      )}
      {status && <p className="agentz-settings-hint">{status}</p>}
      {error && <div className="agentz-settings-error">{error}</div>}
      {current && <PortsPanel />}
    </section>
  );
}
