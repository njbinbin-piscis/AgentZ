import { expect, it } from "vitest";
import {
  detectPythonTemplates,
  expandVars,
  findPythonInterpreter,
  loadLaunchConfigs,
  parseJsonc,
  prepareLaunch,
  type ReadText,
} from "./launchConfigs";

const files = (map: Record<string, string>): ReadText => async (p) => map[p] ?? null;

it("parses comments and trailing commas without touching strings", () => {
  const parsed = parseJsonc(`{
    // line comment
    "a": "x // not a comment, ] }",
    /* block */ "b": [1, 2,],
  }`) as { a: string; b: number[] };
  expect(parsed).toEqual({ a: "x // not a comment, ] }", b: [1, 2] });
});

it("expands workspace and file variables recursively", () => {
  const out = expandVars(
    { program: "${file}", cwd: "${workspaceFolder}/src", args: ["${fileBasenameNoExtension}"], n: 1 },
    { workspaceFolder: "/p", file: "/p/app/main.py" },
  );
  expect(out).toEqual({ program: "/p/app/main.py", cwd: "/p/src", args: ["main"], n: 1 });
});

it("prefers a project virtualenv for the interpreter", async () => {
  expect(await findPythonInterpreter("C:/proj", files({ ".venv/pyvenv.cfg": "" }))).toBe("C:/proj/.venv/Scripts/python.exe");
  expect(await findPythonInterpreter("/proj", files({ "venv/pyvenv.cfg": "" }))).toBe("/proj/venv/bin/python");
  expect(await findPythonInterpreter("/proj", files({}))).toBe("python3");
  expect(await findPythonInterpreter("C:/proj", files({}))).toBe("python");
});

it("detects Flask and FastAPI apps and disables the reloader", async () => {
  const templates = await detectPythonTemplates(
    files({
      "app.py": "from flask import Flask\napp = Flask(__name__)\n",
      "src/main.py": "from fastapi import FastAPI\napi = FastAPI()\n",
    }),
  );
  const flask = templates.find((t) => t.name === "Python: Flask")!;
  expect(flask).toMatchObject({ module: "flask", env: { FLASK_APP: "app.py" } });
  expect(flask.args).toContain("--no-reload");
  const fastapi = templates.find((t) => t.name.startsWith("Python: FastAPI"))!;
  expect(fastapi).toMatchObject({ module: "uvicorn", args: ["main:api", "--port", "8000", "--app-dir", "src"] });
  expect(templates[templates.length - 1]?.name).toBe("Python: Current File");
});

it("lists launch.json entries first and reports invalid JSON", async () => {
  const ok = await loadLaunchConfigs(
    "/p",
    files({ ".vscode/launch.json": `{"configurations":[{"name":"Mine","type":"python","request":"launch","program":"x.py"},]}` }),
  );
  expect(ok.configs[0].name).toBe("Mine");
  expect(ok.error).toBeUndefined();
  const bad = await loadLaunchConfigs("/p", files({ ".vscode/launch.json": "{" }));
  expect(bad.error).toContain("launch.json");
  expect(bad.configs[bad.configs.length - 1]?.name).toBe("Python: Current File");
});

it("points python configs at the interpreter and defaults cwd", () => {
  const out = prepareLaunch(
    { name: "n", type: "python", request: "launch", program: "${file}" },
    { workspaceFolder: "/p", file: "/p/a.py" },
    "/p/.venv/bin/python",
  );
  expect(out).toMatchObject({
    program: "/p/a.py",
    cwd: "/p",
    __adapter: { command: "/p/.venv/bin/python", args: ["-m", "debugpy.adapter"] },
  });
});

it("offers Go and Node templates only for projects that look like them", async () => {
  const { detectGoTemplates, detectNodeTemplates } = await import("./launchConfigs");
  const go = await detectGoTemplates(files({ "go.mod": "module x", "cmd/server/main.go": "package main" }));
  expect(go[0]).toMatchObject({ type: "go", mode: "debug", program: "${workspaceFolder}/cmd/server" });
  expect(go.map((t) => t.name)).toContain("Go: Current File");
  expect(await detectGoTemplates(files({}))).toEqual([]);

  const node = await detectNodeTemplates(
    files({ "package.json": JSON.stringify({ main: "lib/index.js", scripts: { start: "node --enable-source-maps server.js" } }) }),
  );
  expect(node[0]).toMatchObject({ type: "pwa-node", program: "${workspaceFolder}/server.js" });
  expect(node[node.length - 1].name).toBe("Node: Current File");
  expect(await detectNodeTemplates(files({}))).toEqual([]);

  const mixed = await loadLaunchConfigs("/p", files({ "go.mod": "module x", "main.go": "package main" }));
  expect(mixed.configs.some((c) => c.name === "Python: Current File")).toBe(false);
  expect(mixed.configs[0].name).toBe("Go: Launch Package");
});

it("resolves Delve and downloads js-debug only when a Node config needs it", async () => {
  const { prepareLaunchWithAdapter } = await import("./launchConfigs");
  const ctx = { workspaceFolder: "/p", file: null };
  let downloads = 0;
  const ensure = async () => {
    downloads++;
    return "/adapters/dapDebugServer.js";
  };
  const go = await prepareLaunchWithAdapter({ name: "g", type: "go", request: "launch", mode: "debug", program: "/p" }, ctx, "python", ensure);
  expect(go.__adapter).toEqual({ command: "dlv", args: ["dap", "--listen=127.0.0.1:{port}"], tcp: true });
  expect(downloads).toBe(0);
  const node = await prepareLaunchWithAdapter({ name: "n", type: "node", request: "launch", program: "/p/a.js" }, ctx, "python", ensure);
  expect(node.type).toBe("pwa-node");
  expect(node.__adapter).toEqual({ command: "node", args: ["/adapters/dapDebugServer.js", "{port}", "127.0.0.1"], tcp: true });
  expect(downloads).toBe(1);
});
