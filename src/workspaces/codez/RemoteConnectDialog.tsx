import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  authorityOf,
  describeTarget,
  isRemoteDir,
  openProject,
  recentTargets,
  rememberTarget,
  toRemoteDir,
  type RemoteTarget,
} from "../../extensions/remoteTargets";
import "../../components/ProjectTemplateDialog.css";
import "./RemoteConnectDialog.css";

interface RemoteTargets {
  ssh_hosts: string[];
  wsl_distros: string[];
  containers: { id: string; name: string; image: string; status: string }[];
}

interface DevcontainerUp {
  container_id: string;
  remote_user: string | null;
  remote_workspace_folder: string;
}

interface DirListing {
  path: string;
  dirs: string[];
}

type Kind = "ssh" | "docker" | "wsl" | "devcontainer";
type Step = "target" | "auth" | "folder";

const AUTH_ERROR = /permission denied|publickey|authentication|password/i;

function parentDir(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const i = trimmed.lastIndexOf("/");
  return i <= 0 ? "/" : trimmed.slice(0, i);
}

function joinDir(path: string, name: string): string {
  return `${path.replace(/\/+$/, "")}/${name}`;
}

interface RemoteConnectDialogProps {
  projectDir: string | null;
  onClose: () => void;
}

/** Status-bar entry for remote development: pick a host, authenticate once, choose a folder. */
export default function RemoteConnectDialog({ projectDir, onClose }: RemoteConnectDialogProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>("target");
  const [kind, setKind] = useState<Kind>("ssh");
  const [targets, setTargets] = useState<RemoteTargets | null>(null);
  const [recent] = useState(recentTargets);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [target, setTarget] = useState<RemoteTarget | null>(null);
  const [listing, setListing] = useState<DirListing | null>(null);
  const [pathInput, setPathInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<RemoteTargets>("remote_list_targets")
      .then(setTargets)
      .catch(() => setTargets({ ssh_hosts: [], wsl_distros: [], containers: [] }));
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onClose]);

  const run = useCallback(async (label: string, fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setStatus(label);
    try {
      await fn();
      setStatus(null);
    } catch (e) {
      setStatus(null);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  const browse = useCallback(async (tgt: RemoteTarget, path: string | null) => {
    const res = await invoke<DirListing>("remote_list_dirs", { target: tgt, path });
    setListing(res);
    setPathInput(res.path);
  }, []);

  const open = useCallback(
    (tgt: RemoteTarget, dir: string) => {
      rememberTarget(tgt);
      openProject(toRemoteDir(tgt, dir));
      onClose();
    },
    [onClose],
  );

  /** Probe the target; on SSH auth failure ask for the password, else go pick a folder. */
  const reach = useCallback(
    async (tgt: RemoteTarget) => {
      setTarget(tgt);
      try {
        await invoke<string>("remote_probe", { target: tgt });
      } catch (e) {
        if (tgt.kind === "ssh" && AUTH_ERROR.test(String(e))) {
          setStep("auth");
          return;
        }
        throw e;
      }
      await browse(tgt, null);
      setStep("folder");
    },
    [browse],
  );

  const connect = () =>
    void run(t("remote.probing"), async () => {
      if (kind === "devcontainer") {
        if (!projectDir || isRemoteDir(projectDir))
          throw new Error(t("remote.devcontainerLocalOnly"));
        setStatus(t("remote.buildingContainer"));
        const up = await invoke<DevcontainerUp>("remote_devcontainer_up", {
          workspace: projectDir,
        });
        open(
          { kind: "docker", container: up.container_id, user: up.remote_user },
          up.remote_workspace_folder,
        );
        return;
      }
      let tgt: RemoteTarget;
      if (kind === "ssh") {
        let alias = host.trim();
        const portNum = Number(port);
        if (port && portNum !== 22) {
          if (!(portNum > 0 && portNum < 65536)) throw new Error(t("remote.dialog.badPort"));
          const at = alias.lastIndexOf("@");
          alias = await invoke<string>("remote_ssh_add_host", {
            hostname: at < 0 ? alias : alias.slice(at + 1),
            user: at < 0 ? null : alias.slice(0, at),
            port: portNum,
          });
        }
        tgt = { kind: "ssh", host: alias };
      } else if (kind === "wsl") {
        tgt = { kind: "wsl", distro: name.trim() };
      } else {
        tgt = { kind: "docker", container: name.trim() };
      }
      await reach(tgt);
    });

  const installKey = () =>
    void run(t("remote.key.installing"), async () => {
      if (target?.kind !== "ssh") return;
      await invoke<string>("remote_ssh_setup_key", { host: target.host, password });
      setPassword("");
      await reach(target);
    });

  const goto = (path: string) => {
    if (!target) return;
    void run(t("remote.dialog.loadingDir"), () => browse(target, path));
  };

  const suggestions =
    kind === "ssh"
      ? (targets?.ssh_hosts ?? [])
      : kind === "wsl"
        ? (targets?.wsl_distros ?? [])
        : kind === "docker"
          ? (targets?.containers ?? []).map((c) => c.name || c.id)
          : [];
  const canConnect = kind === "devcontainer" || (kind === "ssh" ? !!host.trim() : !!name.trim());

  return (
    <div className="agentz-tpl-overlay" onClick={() => !busy && onClose()}>
      <div className="agentz-tpl-dialog agentz-remote-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="agentz-tpl-header">
          <h2>{t("remote.dialog.title")}</h2>
          <p className="agentz-tpl-sub">
            {step === "target" && t("remote.dialog.targetSub")}
            {step === "auth" &&
              t("remote.dialog.authSub", { host: target?.kind === "ssh" ? target.host : "" })}
            {step === "folder" &&
              target &&
              t("remote.dialog.folderSub", { target: describeTarget(target) })}
          </p>
        </div>

        <div className="agentz-tpl-body">
          {step === "target" && (
            <>
              <div className="agentz-remote-kinds" role="tablist">
                {(["ssh", "devcontainer", "docker", "wsl"] as Kind[]).map((k) => (
                  <button
                    key={k}
                    type="button"
                    role="tab"
                    aria-selected={kind === k}
                    className={kind === k ? "active" : ""}
                    onClick={() => setKind(k)}
                    disabled={busy}
                  >
                    {k === "ssh"
                      ? "SSH"
                      : k === "docker"
                        ? "Docker"
                        : k === "wsl"
                          ? "WSL"
                          : t("remote.devcontainer")}
                  </button>
                ))}
              </div>

              {kind === "ssh" && (
                <div className="agentz-remote-row">
                  <input
                    className="agentz-remote-grow"
                    list="agentz-remote-dialog-hosts"
                    value={host}
                    placeholder={t("remote.placeholder.ssh")}
                    onChange={(e) => setHost(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && canConnect && !busy && connect()}
                    disabled={busy}
                    autoFocus
                  />
                  <input
                    className="agentz-remote-port"
                    value={port}
                    placeholder="22"
                    title={t("remote.dialog.portHint")}
                    onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))}
                    disabled={busy}
                  />
                </div>
              )}
              {(kind === "docker" || kind === "wsl") && (
                <div className="agentz-remote-row">
                  <input
                    className="agentz-remote-grow"
                    list="agentz-remote-dialog-hosts"
                    value={name}
                    placeholder={t(`remote.placeholder.${kind}`)}
                    onChange={(e) => setName(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && canConnect && !busy && connect()}
                    disabled={busy}
                    autoFocus
                  />
                </div>
              )}
              {kind === "devcontainer" && (
                <p className="agentz-tpl-note">{t("remote.dialog.devcontainerHint")}</p>
              )}
              <datalist id="agentz-remote-dialog-hosts">
                {suggestions.map((s) => (
                  <option key={s} value={s} />
                ))}
              </datalist>
              {kind === "ssh" && <p className="agentz-tpl-note">{t("remote.dialog.sshHint")}</p>}

              {recent.length > 0 && (
                <>
                  <div className="agentz-remote-label">{t("remote.dialog.recent")}</div>
                  <ul className="agentz-remote-list">
                    {recent.map((r) => (
                      <li key={authorityOf(r)}>
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => void run(t("remote.probing"), () => reach(r))}
                        >
                          {describeTarget(r)}
                        </button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </>
          )}

          {step === "auth" && (
            <>
              <p className="agentz-tpl-note">{t("remote.dialog.authHint")}</p>
              <div className="agentz-remote-row">
                <input
                  className="agentz-remote-grow"
                  type="password"
                  value={password}
                  placeholder={t("remote.key.password")}
                  onChange={(e) => setPassword(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && password && !busy && installKey()}
                  disabled={busy}
                  autoFocus
                />
              </div>
            </>
          )}

          {step === "folder" && listing && (
            <>
              <div className="agentz-remote-row">
                <input
                  className="agentz-remote-grow"
                  value={pathInput}
                  onChange={(e) => setPathInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && !busy && goto(pathInput)}
                  disabled={busy}
                />
                <button type="button" disabled={busy} onClick={() => goto(pathInput)}>
                  {t("remote.dialog.go")}
                </button>
              </div>
              <ul className="agentz-remote-list agentz-remote-dirs">
                {listing.path !== "/" && (
                  <li>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => goto(parentDir(listing.path))}
                    >
                      ..
                    </button>
                  </li>
                )}
                {listing.dirs.map((d) => (
                  <li key={d}>
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => goto(joinDir(listing.path, d))}
                    >
                      📁 {d}
                    </button>
                  </li>
                ))}
                {listing.dirs.length === 0 && (
                  <li className="agentz-tpl-note">{t("remote.dialog.noDirs")}</li>
                )}
              </ul>
            </>
          )}

          {status && <p className="agentz-tpl-note">{status}</p>}
          {error && <p className="agentz-tpl-error">{error}</p>}
        </div>

        <div className="agentz-tpl-actions">
          {step !== "target" && (
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                setStep("target");
                setError(null);
              }}
            >
              {t("remote.dialog.back")}
            </button>
          )}
          <button type="button" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </button>
          {step === "target" && (
            <button
              type="button"
              className="primary"
              disabled={busy || !canConnect}
              onClick={connect}
            >
              {kind === "devcontainer" ? t("remote.connect") : t("remote.dialog.next")}
            </button>
          )}
          {step === "auth" && (
            <button
              type="button"
              className="primary"
              disabled={busy || !password}
              onClick={installKey}
            >
              {t("remote.dialog.installKey")}
            </button>
          )}
          {step === "folder" && listing && target && (
            <button
              type="button"
              className="primary"
              disabled={busy}
              onClick={() => open(target, listing.path)}
            >
              {t("remote.dialog.openHere")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
