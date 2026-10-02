// Remote workspaces are opened as projects whose dir is
// `agentz-remote://<authority>/<path>`. The authority alone can't say how to
// connect (e.g. docker user), so the full target is remembered per authority.

export type RemoteTarget =
  | { kind: "local" }
  | { kind: "ssh"; host: string }
  | { kind: "docker"; container: string; user?: string | null }
  | { kind: "wsl"; distro: string };

export const REMOTE_PREFIX = "agentz-remote://";

/** App.tsx listens for this and switches the active project. */
export const OPEN_PROJECT_EVENT = "agentz:open-project";

export function openProject(dir: string): void {
  window.dispatchEvent(new CustomEvent<string>(OPEN_PROJECT_EVENT, { detail: dir }));
}
const STORE_KEY = "agentz.remote.targets";

export function authorityOf(target: RemoteTarget): string {
  switch (target.kind) {
    case "ssh":
      return `ssh-remote+${target.host}`;
    case "docker":
      return `docker+${target.container}`;
    case "wsl":
      return `wsl+${target.distro}`;
    default:
      return "local";
  }
}

export function isRemoteDir(dir: string | null | undefined): boolean {
  return !!dir && dir.startsWith(REMOTE_PREFIX);
}

export function parseRemoteDir(dir: string): { authority: string; path: string } | null {
  if (!isRemoteDir(dir)) return null;
  const rest = dir.slice(REMOTE_PREFIX.length);
  const i = rest.indexOf("/");
  const authority = decodeURIComponent(i < 0 ? rest : rest.slice(0, i));
  const path = i < 0 ? "/" : decodeURIComponent(rest.slice(i));
  return authority ? { authority, path } : null;
}

export function toRemoteDir(target: RemoteTarget, path: string): string {
  return `${REMOTE_PREFIX}${authorityOf(target)}${path.startsWith("/") ? path : `/${path}`}`;
}

function load(): Record<string, RemoteTarget> {
  try {
    return JSON.parse(localStorage.getItem(STORE_KEY) ?? "{}") as Record<string, RemoteTarget>;
  } catch {
    return {};
  }
}

export function rememberTarget(target: RemoteTarget): void {
  if (target.kind === "local") return;
  const all = load();
  all[authorityOf(target)] = target;
  localStorage.setItem(STORE_KEY, JSON.stringify(all));
}

export function targetForAuthority(authority: string): RemoteTarget | null {
  const known = load()[authority];
  if (known) return known;
  // SSH and WSL authorities are self-describing.
  if (authority.startsWith("ssh-remote+")) return { kind: "ssh", host: authority.slice("ssh-remote+".length) };
  if (authority.startsWith("wsl+")) return { kind: "wsl", distro: authority.slice("wsl+".length) };
  if (authority.startsWith("docker+")) return { kind: "docker", container: authority.slice("docker+".length) };
  return null;
}

/** Short label for status bars / titles. */
export function describeRemoteDir(dir: string): string | null {
  const parsed = parseRemoteDir(dir);
  if (!parsed) return null;
  const [kind, name] = parsed.authority.split("+", 2);
  const label = kind === "ssh-remote" ? "SSH" : kind === "docker" ? "Container" : kind === "wsl" ? "WSL" : kind;
  return `${label}: ${kind === "docker" ? name.slice(0, 12) : name}`;
}
