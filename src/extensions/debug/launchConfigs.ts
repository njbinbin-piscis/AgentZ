// Debug launch configurations: `.vscode/launch.json` plus auto-detected
// templates (Flask, FastAPI, Django, current Python file) and interpreter
// discovery. Pure functions over an injected `readText`, so they are testable.

export interface LaunchConfig {
  type: string;
  request: string;
  name: string;
  [key: string]: unknown;
}

/** Reads a project-relative or absolute file; resolves to null when it does not exist. */
export type ReadText = (path: string) => Promise<string | null>;

/** Parses JSON with comments and trailing commas (the launch.json dialect). */
export function parseJsonc(text: string): unknown {
  let stripped = "";
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (inString) {
      stripped += c;
      if (c === "\\") stripped += text[++i] ?? "";
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') {
      inString = true;
      stripped += c;
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      stripped += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      i += 2;
      while (i < text.length && !(text[i] === "*" && text[i + 1] === "/")) i++;
      i++;
    } else {
      stripped += c;
    }
  }
  let out = "";
  inString = false;
  for (let i = 0; i < stripped.length; i++) {
    const c = stripped[i];
    if (inString) {
      out += c;
      if (c === "\\") out += stripped[++i] ?? "";
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') inString = true;
    if (c === ",") {
      let j = i + 1;
      while (j < stripped.length && /\s/.test(stripped[j])) j++;
      if (stripped[j] === "}" || stripped[j] === "]") continue;
    }
    out += c;
  }
  return JSON.parse(out);
}

export interface VarContext {
  workspaceFolder: string;
  /** Absolute path of the active editor file, when there is one. */
  file?: string | null;
}

function basename(path: string): string {
  return path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? "";
}
function dirname(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  parts.pop();
  return parts.join("/");
}

/** Expands `${workspaceFolder}`, `${file}` and friends in every string of `value`. */
export function expandVars<T>(value: T, ctx: VarContext): T {
  const expand = (input: string): string => {
    const file = ctx.file ?? "";
    return input
      .replace(/\$\{workspaceFolder\}/g, ctx.workspaceFolder)
      .replace(/\$\{workspaceFolderBasename\}/g, basename(ctx.workspaceFolder))
      .replace(/\$\{file\}/g, file)
      .replace(/\$\{fileBasename\}/g, basename(file))
      .replace(/\$\{fileBasenameNoExtension\}/g, basename(file).replace(/\.[^.]*$/, ""))
      .replace(/\$\{fileDirname\}/g, dirname(file));
  };
  const walk = (v: unknown): unknown => {
    if (typeof v === "string") return expand(v);
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === "object")
      return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
    return v;
  };
  return walk(value) as T;
}

export function isWindowsPath(dir: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(dir);
}

function isRemote(dir: string): boolean {
  return dir.startsWith("agentz-remote://");
}

/** Debug-adapter command for the interpreter the project actually uses. */
export async function findPythonInterpreter(dir: string, readText: ReadText): Promise<string> {
  if (isRemote(dir)) return "python";
  const win = isWindowsPath(dir);
  for (const venv of [".venv", "venv", "env", ".env"]) {
    // pyvenv.cfg is a small text file every virtualenv has, so it is a cheap probe.
    if ((await readText(`${venv}/pyvenv.cfg`)) !== null) {
      return `${dir.replace(/[\\/]+$/, "")}/${venv}/${win ? "Scripts/python.exe" : "bin/python"}`;
    }
  }
  return win ? "python" : "python3";
}

/** Loads `.vscode/launch.json`; returns [] when absent and throws on invalid JSON. */
export async function loadLaunchJson(readText: ReadText): Promise<LaunchConfig[]> {
  const text = await readText(".vscode/launch.json");
  if (text === null) return [];
  const parsed = parseJsonc(text) as { configurations?: unknown };
  const list = Array.isArray(parsed?.configurations) ? parsed.configurations : [];
  return list.filter(
    (c): c is LaunchConfig =>
      !!c && typeof c === "object" && typeof (c as LaunchConfig).type === "string" && typeof (c as LaunchConfig).name === "string",
  ) as LaunchConfig[];
}

const APP_FILES = [
  "app.py",
  "main.py",
  "wsgi.py",
  "asgi.py",
  "server.py",
  "run.py",
  "app/main.py",
  "app/__init__.py",
  "src/main.py",
  "src/app.py",
];

function moduleOf(path: string): string {
  return path.replace(/\/__init__\.py$/, "").replace(/\.py$/, "").replace(/\//g, ".");
}

/** Built-in Python templates that fit what the project looks like. */
export async function detectPythonTemplates(readText: ReadText): Promise<LaunchConfig[]> {
  const templates: LaunchConfig[] = [];
  const manifests = (
    await Promise.all(["requirements.txt", "pyproject.toml", "Pipfile"].map((f) => readText(f)))
  )
    .filter((t): t is string => t !== null)
    .join("\n")
    .toLowerCase();
  const sources = new Map<string, string>();
  for (const file of APP_FILES) {
    const text = await readText(file);
    if (text !== null) sources.set(file, text);
  }

  const flaskFile = [...sources].find(([, text]) => /\bFlask\s*\(/.test(text))?.[0];
  if (flaskFile || manifests.includes("flask")) {
    const target = flaskFile?.endsWith("/__init__.py") ? flaskFile.replace(/\/__init__\.py$/, "") : flaskFile ?? "app.py";
    templates.push({
      name: "Python: Flask",
      type: "python",
      request: "launch",
      module: "flask",
      args: ["run", "--no-debugger", "--no-reload"],
      env: { FLASK_APP: target, FLASK_DEBUG: "0" },
      jinja: true,
      justMyCode: true,
    });
  }

  let fastapi: { file: string; variable: string } | undefined;
  for (const [file, text] of sources) {
    const m = /^(\w+)\s*(?::[^=\n]+)?=\s*FastAPI\s*\(/m.exec(text);
    if (m) {
      fastapi = { file, variable: m[1] };
      break;
    }
  }
  if (fastapi || manifests.includes("fastapi")) {
    const file = fastapi?.file ?? "main.py";
    const underSrc = file.startsWith("src/");
    const module = moduleOf(underSrc ? file.slice(4) : file);
    templates.push({
      name: "Python: FastAPI (uvicorn)",
      type: "python",
      request: "launch",
      module: "uvicorn",
      args: [`${module}:${fastapi?.variable ?? "app"}`, "--port", "8000", ...(underSrc ? ["--app-dir", "src"] : [])],
      justMyCode: true,
    });
  }

  if ((await readText("manage.py")) !== null) {
    templates.push({
      name: "Python: Django",
      type: "python",
      request: "launch",
      program: "${workspaceFolder}/manage.py",
      args: ["runserver", "--noreload"],
      django: true,
      justMyCode: true,
    });
  }

  templates.push({
    name: "Python: Current File",
    type: "python",
    request: "launch",
    program: "${file}",
    justMyCode: true,
  });
  return templates;
}

/** Go: a package launch (main package located by convention), the current file, and package tests. */
export async function detectGoTemplates(readText: ReadText): Promise<LaunchConfig[]> {
  if ((await readText("go.mod")) === null) return [];
  const templates: LaunchConfig[] = [];
  for (const candidate of ["main.go", "cmd/server/main.go", "cmd/api/main.go", "cmd/app/main.go", "cmd/main.go"]) {
    if ((await readText(candidate)) === null) continue;
    const dir = candidate.includes("/") ? candidate.slice(0, candidate.lastIndexOf("/")) : "";
    templates.push({
      name: dir ? `Go: Launch ${dir}` : "Go: Launch Package",
      type: "go",
      request: "launch",
      mode: "debug",
      program: dir ? "${workspaceFolder}/" + dir : "${workspaceFolder}",
    });
    break;
  }
  templates.push(
    { name: "Go: Current File", type: "go", request: "launch", mode: "debug", program: "${file}" },
    { name: "Go: Test Current Package", type: "go", request: "launch", mode: "test", program: "${fileDirname}" },
  );
  return templates;
}

/** Node.js: the package entry point, a start script's file, and the current file. */
export async function detectNodeTemplates(readText: ReadText): Promise<LaunchConfig[]> {
  const manifest = await readText("package.json");
  if (manifest === null) return [];
  let entry: string | undefined;
  try {
    const pkg = parseJsonc(manifest) as { main?: unknown; scripts?: Record<string, unknown> };
    const fromStart = typeof pkg.scripts?.start === "string" ? /\bnode\s+(?:--\S+\s+)*([\w./-]+\.[cm]?js)\b/.exec(pkg.scripts.start)?.[1] : undefined;
    entry = fromStart ?? (typeof pkg.main === "string" ? pkg.main : undefined);
  } catch {
    /* an unreadable package.json still allows "current file" */
  }
  if (!entry) {
    for (const candidate of ["index.js", "server.js", "app.js", "src/index.js", "src/server.js", "src/app.js"]) {
      if ((await readText(candidate)) !== null) {
        entry = candidate;
        break;
      }
    }
  }
  const base = { type: "pwa-node", request: "launch", cwd: "${workspaceFolder}", skipFiles: ["<node_internals>/**"], console: "internalConsole" };
  const templates: LaunchConfig[] = [];
  if (entry) {
    templates.push({ name: `Node: Launch ${entry}`, ...base, program: "${workspaceFolder}/" + entry.replace(/^\.\//, "") });
  }
  templates.push({ name: "Node: Current File", ...base, program: "${file}" });
  return templates;
}

/** launch.json entries first, then built-in templates whose names are not already taken. */
export async function loadLaunchConfigs(dir: string, readText: ReadText): Promise<{ configs: LaunchConfig[]; error?: string }> {
  let configs: LaunchConfig[] = [];
  let error: string | undefined;
  try {
    configs = await loadLaunchJson(readText);
  } catch (e) {
    error = `.vscode/launch.json: ${e instanceof Error ? e.message : String(e)}`;
  }
  const taken = new Set(configs.map((c) => c.name));
  const python = await detectPythonTemplates(readText).catch(() => []);
  const go = await detectGoTemplates(readText).catch(() => []);
  const node = await detectNodeTemplates(readText).catch(() => []);
  // A lone "Python: Current File" is only a fallback; do not offer it in a Go or Node project.
  const detected = [...(python.length === 1 && (go.length || node.length) ? [] : python), ...go, ...node];
  for (const t of detected) if (!taken.has(t.name)) configs.push(t);
  void dir;
  return { configs, error };
}

/** Which debug adapter downloads a configuration needs. */
export type EnsureAdapter = (name: "js-debug") => Promise<string>;

/**
 * Like `prepareLaunch`, and also resolves adapters that are not plain commands:
 * Delve for Go, and js-debug (downloaded on first use) for Node.js.
 */
export async function prepareLaunchWithAdapter(
  config: LaunchConfig,
  ctx: VarContext,
  interpreter: string,
  ensureAdapter: EnsureAdapter,
): Promise<LaunchConfig> {
  const resolved = prepareLaunch(config, ctx, interpreter);
  if ((resolved.type === "go" || resolved.type === "delve") && !resolved.__adapter) {
    resolved.type = "go";
    resolved.__adapter = { command: "dlv", args: ["dap", "--listen=127.0.0.1:{port}"], tcp: true };
  }
  if (resolved.type === "node" || resolved.type === "pwa-node") {
    resolved.type = "pwa-node";
    if (!resolved.__adapter) {
      const entry = await ensureAdapter("js-debug");
      resolved.__adapter = { command: "node", args: [entry, "{port}", "127.0.0.1"], tcp: true };
    }
  }
  return resolved;
}

/**
 * Turns a config into something the controller can run: expands variables,
 * defaults `cwd`, and points the adapter at the project's interpreter.
 */
export function prepareLaunch(config: LaunchConfig, ctx: VarContext, interpreter: string): LaunchConfig {
  const resolved = expandVars(config, ctx);
  if (resolved.request !== "attach" && resolved.cwd === undefined) resolved.cwd = ctx.workspaceFolder;
  if ((resolved.type === "python" || resolved.type === "debugpy") && !resolved.__adapter) {
    resolved.__adapter = { command: interpreter, args: ["-m", "debugpy.adapter"] };
  }
  return resolved;
}
