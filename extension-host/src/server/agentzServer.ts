// agentz-server control plane. Lines carrying a `$agentz` key share the
// extension host's stdio but are handled here instead of by the RPC protocol,
// so the Rust broker can do FS / process work on whichever machine the host
// runs on (local, SSH remote, container, WSL).

import * as fs from "node:fs/promises";
import * as os from "node:os";
import * as path from "node:path";
import { spawn, ChildProcess } from "node:child_process";
import { watch as watchFs, FSWatcher } from "node:fs";
import { connect, Socket } from "node:net";
import { EventEmitter } from "node:events";

const MAX_WATCHED_DIRS = 8000;

/**
 * Recursive watcher built from per-directory (non-recursive) watches, skipping
 * ignored/hidden directories. New directories are picked up as they appear.
 */
class PrunedTreeWatcher extends EventEmitter {
  private dirs = new Map<string, FSWatcher>();
  private closed = false;

  constructor(
    private root: string,
    private onChange: (rel: string) => void,
  ) {
    super();
    void this.add(root);
  }

  private async add(dir: string): Promise<void> {
    if (this.closed || this.dirs.has(dir)) return;
    if (this.dirs.size >= MAX_WATCHED_DIRS) {
      if (this.dirs.size === MAX_WATCHED_DIRS) this.emit("error", new Error(`watch limit (${MAX_WATCHED_DIRS} dirs) reached`));
      return;
    }
    let w: FSWatcher;
    try {
      w = watchFs(dir, (_type, filename) => {
        if (!filename) return;
        const full = path.posix.join(dir, filename.toString());
        this.onChange(path.posix.relative(this.root, full));
        fs.stat(full).then(
          (s) => {
            if (s.isDirectory() && !skipDir(filename.toString())) void this.add(full);
          },
          () => this.remove(full),
        );
      });
    } catch {
      return;
    }
    w.on("error", () => this.remove(dir));
    this.dirs.set(dir, w);
    let entries: import("node:fs").Dirent[];
    try {
      entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      if (e.isDirectory() && !skipDir(e.name)) await this.add(path.posix.join(dir, e.name));
    }
  }

  private remove(dir: string): void {
    for (const [d, w] of this.dirs) {
      if (d === dir || d.startsWith(dir + "/")) {
        w.close();
        this.dirs.delete(d);
      }
    }
  }

  close(): void {
    this.closed = true;
    for (const w of this.dirs.values()) w.close();
    this.dirs.clear();
  }
}

function skipDir(name: string): boolean {
  return IGNORED_DIR_NAMES.has(name) || name.startsWith(".");
}

/** Workspace-relative roots of git repos ("" = the workspace itself). */
async function discoverGitRepos(root: string, maxDepth: number): Promise<string[]> {
  const found: string[] = [];
  const walk = async (dir: string, depth: number): Promise<void> => {
    try {
      await fs.stat(path.posix.join(dir, ".git"));
      found.push(path.posix.relative(root, dir));
      return;
    } catch {
      /* not a repo root */
    }
    if (depth >= maxDepth) return;
    let entries: import("node:fs").Dirent[];
    try {
      entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    await Promise.all(
      entries
        .filter((e) => e.isDirectory() && !IGNORED_DIR_NAMES.has(e.name) && !e.name.startsWith("."))
        .map((e) => walk(path.posix.join(dir, e.name), depth + 1)),
    );
  };
  await walk(root, 0);
  return found.sort();
}

interface ListeningPort {
  port: number;
  address: string;
  process: string | null;
}

// Linux: parse /proc/net/tcp{,6} (no ss/netstat dependency); state 0A = LISTEN.
async function listListeningPorts(): Promise<ListeningPort[]> {
  const seen = new Map<number, ListeningPort>();
  for (const file of ["/proc/net/tcp", "/proc/net/tcp6"]) {
    let text: string;
    try {
      text = await fs.readFile(file, "utf8");
    } catch {
      continue;
    }
    for (const line of text.split("\n").slice(1)) {
      const cols = line.trim().split(/\s+/);
      if (cols.length < 4 || cols[3] !== "0A") continue;
      const [addrHex, portHex] = cols[1].split(":");
      const port = parseInt(portHex, 16);
      if (!port || seen.has(port)) continue;
      const loopback = addrHex === "0100007F" || /^0{24}01000000$|^0{31}1$/i.test(addrHex);
      seen.set(port, { port, address: loopback ? "localhost" : "any", process: null });
    }
  }
  return [...seen.values()].sort((a, b) => a.port - b.port);
}

interface TreeNode {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: string;
  children: TreeNode[] | null;
}

const IGNORED_DIR_NAMES = new Set([
  ".git", "node_modules", "target", "dist", "build", ".next", ".nuxt", "__pycache__",
  ".venv", "venv", ".idea", ".vscode-test", ".cache", ".DS_Store", ".gradle", ".turbo",
]);

function isIgnoredTreeEntry(name: string, rel: string, patterns: string[]): boolean {
  if (IGNORED_DIR_NAMES.has(name)) return true;
  for (const pattern of patterns) {
    const p = pattern.replace(/^\/+/, "");
    if (name === p || rel.includes(p)) return true;
    if (p.endsWith("/") && name === p.replace(/\/+$/, "")) return true;
    if (p.startsWith("*") && name.endsWith(p.replace(/^\*+/, ""))) return true;
  }
  return false;
}

interface SearchParams {
  root: string;
  query: string;
  filePattern?: string | null;
  excludePattern?: string | null;
  caseSensitive?: boolean;
  wholeWord?: boolean;
  useRegex?: boolean;
  maxResults?: number;
}

interface SearchHit {
  path: string;
  line: number;
  column: number;
  text: string;
  context_before: string | null;
  context_after: string | null;
}

function run(cmd: string, args: string[], cwd: string): Promise<{ code: number | null; stdout: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, windowsHide: true });
    let stdout = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout }));
  });
}

// ripgrep when installed on the remote, otherwise GNU/BSD grep.
async function search(p: SearchParams): Promise<SearchHit[]> {
  const max = p.maxResults ?? 1000;
  const hits: SearchHit[] = [];
  const rgArgs = ["--json", "--max-count", "200"];
  if (!p.caseSensitive) rgArgs.push("-i");
  if (p.wholeWord) rgArgs.push("-w");
  if (!p.useRegex) rgArgs.push("-F");
  if (p.filePattern) for (const g of p.filePattern.split(",")) rgArgs.push("-g", g.trim());
  if (p.excludePattern) for (const g of p.excludePattern.split(",")) rgArgs.push("-g", `!${g.trim()}`);
  rgArgs.push("--", p.query, ".");
  try {
    const { stdout } = await run("rg", rgArgs, p.root);
    for (const line of stdout.split("\n")) {
      if (hits.length >= max) break;
      if (!line.startsWith('{"type":"match"')) continue;
      const m = JSON.parse(line).data;
      hits.push({
        path: String(m.path.text).replace(/\\/g, "/").replace(/^\.\//, ""),
        line: m.line_number,
        column: (m.submatches?.[0]?.start ?? 0) + 1,
        text: String(m.lines.text ?? "").replace(/\r?\n$/, ""),
        context_before: null,
        context_after: null,
      });
    }
    return hits;
  } catch {
    /* rg missing: fall through */
  }
  const grepArgs = ["-rnI", "--exclude-dir=.git", "--exclude-dir=node_modules"];
  if (!p.caseSensitive) grepArgs.push("-i");
  if (p.wholeWord) grepArgs.push("-w");
  grepArgs.push(p.useRegex ? "-E" : "-F");
  if (p.filePattern) for (const g of p.filePattern.split(",")) grepArgs.push(`--include=${g.trim()}`);
  if (p.excludePattern) for (const g of p.excludePattern.split(",")) grepArgs.push(`--exclude=${g.trim()}`);
  grepArgs.push("--", p.query, ".");
  const { stdout } = await run("grep", grepArgs, p.root);
  for (const line of stdout.split("\n")) {
    if (hits.length >= max) break;
    const m = /^(.*?):(\d+):(.*)$/.exec(line);
    if (!m) continue;
    const text = m[3];
    const idx = p.caseSensitive ? text.indexOf(p.query) : text.toLowerCase().indexOf(p.query.toLowerCase());
    hits.push({ path: m[1].replace(/^\.\//, ""), line: Number(m[2]), column: Math.max(0, idx) + 1, text, context_before: null, context_after: null });
  }
  return hits;
}

export interface ControlRequest {
  $agentz: "req";
  id: number;
  method: string;
  params?: Record<string, unknown>;
}

type Emit = (frame: Record<string, unknown>) => void;

const MAX_READ_BYTES = 64 * 1024 * 1024;

export class AgentzServer {
  private procs = new Map<number, ChildProcess>();
  private nextProc = 1;

  constructor(private readonly emit: Emit) {
    process.on("exit", () => {
      for (const child of this.procs.values()) child.kill();
    });
  }

  static isControl(msg: unknown): msg is ControlRequest {
    return typeof msg === "object" && msg !== null && (msg as { $agentz?: unknown }).$agentz === "req";
  }

  async handle(req: ControlRequest): Promise<void> {
    try {
      const result = await this.dispatch(req.method, req.params ?? {});
      this.emit({ $agentz: "res", id: req.id, result: result ?? null });
    } catch (err) {
      const e = err as NodeJS.ErrnoException;
      this.emit({ $agentz: "res", id: req.id, error: { message: e?.message ?? String(err), code: e?.code ?? null } });
    }
  }

  private async dispatch(method: string, p: Record<string, unknown>): Promise<unknown> {
    const str = (k: string): string => {
      const v = p[k];
      if (typeof v !== "string") throw new Error(`param '${k}' must be a string`);
      return v;
    };
    switch (method) {
      case "info":
        return {
          platform: process.platform,
          arch: process.arch,
          node: process.version,
          home: os.homedir(),
          hostname: os.hostname(),
          cwd: process.cwd(),
          pathSep: path.sep,
        };
      case "fs.stat": {
        const s = await fs.stat(str("path"));
        return { size: s.size, mtimeMs: s.mtimeMs, isFile: s.isFile(), isDirectory: s.isDirectory() };
      }
      case "fs.readFile": {
        const file = str("path");
        const s = await fs.stat(file);
        if (s.size > MAX_READ_BYTES) throw new Error(`file too large: ${s.size} bytes`);
        return { base64: (await fs.readFile(file)).toString("base64") };
      }
      case "fs.writeFile": {
        const file = str("path");
        await fs.mkdir(path.dirname(file), { recursive: true });
        const data = typeof p.base64 === "string" ? Buffer.from(p.base64, "base64") : Buffer.from(str("text"), "utf8");
        await fs.writeFile(file, data);
        return true;
      }
      case "fs.readDir": {
        const entries = await fs.readdir(str("path"), { withFileTypes: true });
        return entries.map((e) => ({ name: e.name, isDirectory: e.isDirectory(), isFile: e.isFile(), isSymlink: e.isSymbolicLink() }));
      }
      case "fs.mkdir":
        await fs.mkdir(str("path"), { recursive: true });
        return true;
      case "fs.delete":
        await fs.rm(str("path"), { recursive: p.recursive === true, force: true });
        return true;
      case "fs.rename":
        await fs.rename(str("from"), str("to"));
        return true;
      case "fs.tree":
        return this.tree(str("path"), (p.depth as number) ?? 10, (p.maxEntries as number) ?? 50_000);
      case "fs.watch":
        return this.watch(str("path"), str("token"));
      case "fs.unwatch":
        this.watchers.get(str("token"))?.close();
        this.watchers.delete(str("token"));
        return true;
      case "search":
        return search(p as unknown as SearchParams);
      case "git.discover":
        return discoverGitRepos(str("path"), (p.maxDepth as number) ?? 4);
      case "ports.list":
        return listListeningPorts();
      case "tcp.connect":
        return this.tcpConnect((p.host as string) || "127.0.0.1", p.port as number);
      case "tcp.write": {
        this.sockets.get(p.id as number)?.write(Buffer.from(str("base64"), "base64"));
        return true;
      }
      case "tcp.close":
        this.sockets.get(p.id as number)?.destroy();
        this.sockets.delete(p.id as number);
        return true;
      case "exec":
        return this.exec(str("command"), (p.args as string[]) ?? [], p.cwd as string | undefined, (p.timeoutMs as number) ?? 120_000);
      case "proc.spawn":
        return this.spawnStreaming(str("command"), (p.args as string[]) ?? [], p.cwd as string | undefined, p.env as Record<string, string> | undefined);
      case "proc.write": {
        const child = this.procs.get(p.pid as number);
        if (!child?.stdin) throw new Error("no such process");
        child.stdin.write(str("data"));
        return true;
      }
      case "proc.kill": {
        this.procs.get(p.pid as number)?.kill();
        return true;
      }
      default:
        throw new Error(`unknown agentz method: ${method}`);
    }
  }

  private watchers = new Map<string, { close(): void }>();
  private sockets = new Map<number, Socket>();
  private nextSocket = 1;

  // Port-forward tunnel endpoint for transports without native forwarding.
  private tcpConnect(host: string, port: number): Promise<{ id: number }> {
    return new Promise((resolve, reject) => {
      const id = this.nextSocket++;
      const sock = connect({ host, port }, () => {
        this.sockets.set(id, sock);
        resolve({ id });
      });
      sock.on("data", (d) => this.emit({ $agentz: "event", event: "tcp.data", id, base64: d.toString("base64") }));
      sock.on("close", () => {
        this.sockets.delete(id);
        this.emit({ $agentz: "event", event: "tcp.close", id });
      });
      sock.on("error", (e) => {
        if (!this.sockets.has(id)) reject(e);
      });
    });
  }

  // Mirrors the desktop tree builder (`path_filter::is_ignored_tree_entry`).
  private async tree(root: string, maxDepth: number, maxEntries: number): Promise<TreeNode[]> {
    let patterns: string[] = [];
    try {
      patterns = (await fs.readFile(path.posix.join(root, ".gitignore"), "utf8"))
        .split(/\r?\n/)
        .map((l) => l.trim())
        .filter((l) => l && !l.startsWith("#"));
    } catch {
      /* no .gitignore */
    }
    let budget = maxEntries;
    const walk = async (dir: string, depth: number): Promise<TreeNode[]> => {
      if (depth >= maxDepth || budget <= 0) return [];
      let entries: import("node:fs").Dirent[];
      try {
        entries = await fs.readdir(dir, { withFileTypes: true });
      } catch {
        return [];
      }
      const nodes: TreeNode[] = [];
      for (const e of entries) {
        if (budget-- <= 0) break;
        const full = path.posix.join(dir, e.name);
        const rel = path.posix.relative(root, full);
        if (isIgnoredTreeEntry(e.name, rel, patterns)) continue;
        let st: import("node:fs").Stats;
        try {
          st = await fs.stat(full);
        } catch {
          continue;
        }
        const modified = new Date(st.mtimeMs).toISOString();
        if (st.isDirectory()) {
          nodes.push({ name: e.name, path: rel, is_dir: true, size: 0, modified, children: await walk(full, depth + 1) });
        } else {
          nodes.push({ name: e.name, path: rel, is_dir: false, size: st.size, modified, children: null });
        }
      }
      return nodes;
    };
    return walk(root, 0);
  }

  private watch(root: string, token: string): boolean {
    if (this.watchers.has(token)) return true;
    const pending = new Map<string, NodeJS.Timeout>();
    const onChange = (rel: string) => {
      if (/(^|\/)(\.git|node_modules)(\/|$)/.test(rel)) return;
      // Coalesce editor save bursts (write + rename + chmod) into one event.
      clearTimeout(pending.get(rel));
      pending.set(
        rel,
        setTimeout(() => {
          pending.delete(rel);
          fs.stat(path.posix.join(root, rel)).then(
            () => this.emit({ $agentz: "event", event: "fs.change", token, path: rel, kind: "modified" }),
            () => this.emit({ $agentz: "event", event: "fs.change", token, path: rel, kind: "deleted" }),
          );
        }, 100),
      );
    };
    // Node's recursive fs.watch on Linux is emulated in JS: it walks and
    // watches *everything* (node_modules included) and can pin a core for
    // minutes. Watch pruned directories one inotify watch each instead.
    const watcher =
      process.platform === "linux"
        ? new PrunedTreeWatcher(root, onChange)
        : watchFs(root, { recursive: true }, (_type, filename) => {
            if (filename) onChange(filename.toString().replace(/\\/g, "/"));
          });
    watcher.on("error", (e: Error) => process.stderr.write(`[server] watch ${root}: ${e.message}\n`));
    this.watchers.set(token, watcher);
    return true;
  }

  private exec(command: string, args: string[], cwd: string | undefined, timeoutMs: number): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const child = spawn(command, args, { cwd, shell: args.length === 0, windowsHide: true });
      let stdout = "";
      let stderr = "";
      const timer = setTimeout(() => child.kill(), timeoutMs);
      child.stdout?.on("data", (d) => (stdout += d));
      child.stderr?.on("data", (d) => (stderr += d));
      child.on("error", (e) => {
        clearTimeout(timer);
        reject(e);
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        resolve({ code, stdout, stderr });
      });
    });
  }

  // Pipe-based (not a real PTY): enough for tasks and simple shells on remotes
  // without shipping a native node-pty build per platform.
  private spawnStreaming(command: string, args: string[], cwd: string | undefined, env: Record<string, string> | undefined): { pid: number } {
    const pid = this.nextProc++;
    const child = spawn(command, args, { cwd, env: { ...process.env, ...env }, shell: args.length === 0, windowsHide: true });
    this.procs.set(pid, child);
    const out = (stream: "stdout" | "stderr") => (d: Buffer) =>
      this.emit({ $agentz: "event", event: "proc.data", pid, stream, data: d.toString("utf8") });
    child.stdout?.on("data", out("stdout"));
    child.stderr?.on("data", out("stderr"));
    child.on("close", (code) => {
      this.procs.delete(pid);
      this.emit({ $agentz: "event", event: "proc.exit", pid, code });
    });
    child.on("error", (e) => this.emit({ $agentz: "event", event: "proc.exit", pid, code: -1, error: e.message }));
    return { pid };
  }
}
