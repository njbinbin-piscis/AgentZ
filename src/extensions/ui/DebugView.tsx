import { useEffect, useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { debugStore, type Variable } from "../debug/debugStore";
import { debugController } from "../debug/debugController";
import { activeEditorFile } from "../debug/breakpoints";
import {
  findPythonInterpreter,
  loadLaunchConfigs,
  prepareLaunchWithAdapter,
  type LaunchConfig,
  type ReadText,
} from "../debug/launchConfigs";
import { extensionUiStore } from "../extensionUiStore";
import { invoke } from "@tauri-apps/api/core";
import { ideApi } from "../../services/tauri/ide";
import { useProjectEdgeActions } from "../../contexts/ProjectEdgeContext";
import { useExtensionUi } from "./useExtensionUi";

// Watch expressions survive tab switches and sessions.
let savedWatches: string[] = [];

function makeReadText(dir: string): ReadText {
  return async (relative) => {
    try {
      const file = await ideApi.readFile(`${dir.replace(/[\\/]+$/, "")}/${relative}`);
      return file.is_binary ? null : file.content;
    } catch {
      return null;
    }
  };
}

/** One variable (or evaluated expression) that can be expanded when it has children. */
function VarNode({ name, value, reference }: { name: string; value: string; reference: number }) {
  const [open, setOpen] = useState(false);
  const [children, setChildren] = useState<Variable[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && children === null) {
      debugController
        .getChildren(reference)
        .then(setChildren)
        .catch((e) => setFailed(String(e)));
    }
  };

  return (
    <div className="agentz-debug-node">
      <div className="agentz-debug-node-row agentz-debug-var">
        {reference > 0 ? (
          <button type="button" className="agentz-debug-twisty" onClick={toggle} aria-expanded={open}>
            {open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="agentz-debug-twisty-spacer" />
        )}
        <span className="agentz-debug-var-name">{name}</span>
        <span className="agentz-debug-var-val">{value}</span>
      </div>
      {open && (
        <div className="agentz-debug-node-children">
          {failed && <div className="agentz-debug-error">{failed}</div>}
          {children === null && !failed && <div className="agentz-debug-muted">…</div>}
          {children?.map((c) => (
            <VarNode key={c.name} name={c.name} value={c.value} reference={c.variablesReference} />
          ))}
        </div>
      )}
    </div>
  );
}

type WatchResult = { value: string; reference: number } | { error: string } | null;

function WatchRow({ expression, stopSeq, paused, onRemove }: {
  expression: string;
  stopSeq: number;
  paused: boolean;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  const [result, setResult] = useState<WatchResult>(null);
  useEffect(() => {
    if (!paused) return;
    let current = true;
    debugController
      .evaluate(expression, "watch")
      .then((r) => current && setResult({ value: r.result, reference: r.variablesReference }))
      .catch((e) => current && setResult({ error: e instanceof Error ? e.message : String(e) }));
    return () => {
      current = false;
    };
  }, [expression, stopSeq, paused]);

  return (
    <div className="agentz-debug-node-row">
      <div style={{ flex: 1, minWidth: 0 }}>
        {result && "value" in result ? (
          <VarNode key={`${stopSeq}:${expression}`} name={expression} value={result.value} reference={result.reference} />
        ) : (
          <div className="agentz-debug-node-row agentz-debug-var">
            <span className="agentz-debug-twisty-spacer" />
            <span className="agentz-debug-var-name">{expression}</span>
            <span className="agentz-debug-muted">{result && "error" in result ? result.error : "—"}</span>
          </div>
        )}
      </div>
      <button type="button" className="agentz-debug-watch-remove" onClick={onRemove} title={t("debug.removeWatch")}>
        ×
      </button>
    </div>
  );
}

function LaunchBar({ projectDir }: { projectDir: string }) {
  const { t } = useTranslation();
  const [configs, setConfigs] = useState<LaunchConfig[]>([]);
  const [selected, setSelected] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    let current = true;
    void loadLaunchConfigs(projectDir, makeReadText(projectDir)).then(({ configs: list, error: problem }) => {
      if (!current) return;
      setConfigs(list);
      setSelected(0);
      setError(problem ?? null);
    });
    return () => {
      current = false;
    };
  }, [projectDir]);

  const start = async () => {
    const config = configs[selected];
    if (!config) return;
    setError(null);
    if (/\$\{file/.test(JSON.stringify(config)) && !activeEditorFile.path) {
      setError(t("debug.needFile"));
      return;
    }
    setStarting(true);
    try {
      const interpreter = await findPythonInterpreter(projectDir, makeReadText(projectDir));
      const launch = await prepareLaunchWithAdapter(
        config,
        { workspaceFolder: projectDir, file: activeEditorFile.path },
        interpreter,
        async (name) => {
          extensionUiStore.appendDebugOutput(`Preparing debug adapter ${name} (downloaded on first use)…`);
          return invoke<string>("debug_adapter_ensure", { name });
        },
        (entry, workspace) => invoke("debug_adapter_publish_remote", { entry, workspace }),
      );
      await debugController.start(launch);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setStarting(false);
    }
  };

  return (
    <div>
      <div className="agentz-debug-launch">
        <select value={selected} onChange={(e) => setSelected(Number(e.target.value))} disabled={configs.length === 0}>
          {configs.length === 0 && <option value={0}>{t("debug.noConfigs")}</option>}
          {configs.map((c, i) => (
            <option key={`${i}:${c.name}`} value={i}>
              {c.name}
            </option>
          ))}
        </select>
        <button type="button" className="agentz-debug-start" onClick={() => void start()} disabled={starting || configs.length === 0}>
          {starting ? t("debug.starting") : `▶ ${t("debug.start")}`}
        </button>
        <span className="agentz-debug-idle">{t("debug.hint")}</span>
      </div>
      {error && <div className="agentz-debug-error">{error}</div>}
    </div>
  );
}

/** Debug session view: launch bar / toolbar + call stack + variables + watch + console. */
export default function DebugView({ projectDir }: { projectDir?: string | null } = {}) {
  const { t } = useTranslation();
  const dbg = useSyncExternalStore(debugStore.subscribe, debugStore.getSnapshot);
  const { debugOutput } = useExtensionUi();
  const { onSelectPath } = useProjectEdgeActions();
  const [watches, setWatches] = useState<string[]>(savedWatches);
  const [watchDraft, setWatchDraft] = useState("");
  const [replDraft, setReplDraft] = useState("");
  const paused = dbg.active && !dbg.running;

  const updateWatches = (next: string[]) => {
    savedWatches = next;
    setWatches(next);
  };

  const runRepl = async () => {
    const expression = replDraft.trim();
    if (!expression) return;
    setReplDraft("");
    extensionUiStore.appendDebugOutput(`> ${expression}`);
    try {
      const r = await debugController.evaluate(expression, "repl");
      extensionUiStore.appendDebugOutput(r.result);
    } catch (e) {
      extensionUiStore.appendDebugOutput(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="agentz-debug-view">
      <div className="agentz-debug-toolbar">
        {dbg.active ? (
          <>
            <button onClick={() => void debugController.continue()} disabled={dbg.running} title={t("debug.continue")}>
              ▶
            </button>
            <button onClick={() => void debugController.stepOver()} disabled={dbg.running} title={t("debug.stepOver")}>
              ⤼
            </button>
            <button onClick={() => void debugController.stepInto()} disabled={dbg.running} title={t("debug.stepInto")}>
              ⤓
            </button>
            <button onClick={() => void debugController.stepOut()} disabled={dbg.running} title={t("debug.stepOut")}>
              ⤒
            </button>
            <button onClick={() => void debugController.stop()} title={t("debug.stop")} className="agentz-debug-stop">
              ■
            </button>
            <span className="agentz-debug-state">{dbg.running ? t("debug.running") : t("debug.paused")}</span>
          </>
        ) : projectDir ? (
          <LaunchBar projectDir={projectDir} />
        ) : (
          <span className="agentz-debug-idle">{t("debug.needProject")}</span>
        )}
      </div>

      {dbg.active && (
        <div className="agentz-debug-cols">
          <div className="agentz-debug-col">
            <div className="agentz-ext-view-title">{t("debug.callStack")}</div>
            {dbg.frames.length === 0 ? (
              <div className="agentz-debug-muted">—</div>
            ) : (
              dbg.frames.map((f) => {
                const relative = debugController.projectRelativePath(f.source);
                return (
                  <div key={f.id} className="agentz-debug-frame">
                    {relative ? (
                      <button
                        type="button"
                        className="agentz-debug-frame-open"
                        title={t("debug.openFrame", { path: relative })}
                        onClick={() => onSelectPath(relative)}
                      >
                        {f.name}
                      </button>
                    ) : (
                      f.name
                    )}{" "}
                    <span className="agentz-debug-muted">:{f.line}</span>
                    {!relative && (
                      <span className="agentz-debug-muted">
                        {" "}
                        ({f.source ? t("debug.outsideProject") : t("debug.noSource")})
                      </span>
                    )}
                  </div>
                );
              })
            )}
          </div>
          <div className="agentz-debug-col">
            <div className="agentz-ext-view-title">{t("debug.variables")}</div>
            {dbg.variables.length === 0 ? (
              <div className="agentz-debug-muted">—</div>
            ) : (
              dbg.variables.map((v) => (
                <VarNode key={`${dbg.stopSeq}:${v.name}`} name={v.name} value={v.value} reference={v.variablesReference} />
              ))
            )}
          </div>
          <div className="agentz-debug-col">
            <div className="agentz-ext-view-title">{t("debug.watch")}</div>
            {watches.map((w, i) => (
              <WatchRow
                key={`${i}:${w}`}
                expression={w}
                stopSeq={dbg.stopSeq}
                paused={paused}
                onRemove={() => updateWatches(watches.filter((_, j) => j !== i))}
              />
            ))}
            <input
              className="agentz-debug-input"
              value={watchDraft}
              placeholder={t("debug.watchPlaceholder")}
              onChange={(e) => setWatchDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && watchDraft.trim()) {
                  updateWatches([...watches, watchDraft.trim()]);
                  setWatchDraft("");
                }
              }}
            />
          </div>
        </div>
      )}

      <div className="agentz-ext-view-title">{t("ide.debugConsole")}</div>
      <pre className="agentz-ext-debug">{debugOutput.length === 0 ? t("debug.noOutput") : debugOutput.join("\n")}</pre>
      {paused && (
        <input
          className="agentz-debug-input"
          value={replDraft}
          placeholder={t("debug.evalPlaceholder")}
          onChange={(e) => setReplDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void runRepl();
          }}
        />
      )}
    </div>
  );
}
