import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectScope } from "../../services/tauri/projectScope";
import { observeTaskView, type TaskViewObservation } from "../../services/tauri/taskView";
import {
  listTaskFiles,
  readTaskFile,
  type TaskFileList,
  type TaskFileText,
} from "../../services/tauri/taskFiles";

export default function TaskFileViewer({
  projectDir,
  sessionId,
  onClose,
}: {
  projectDir: string;
  sessionId: string;
  onClose(): void;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => (zh ? cn : en);
  const scope = useProjectScope(projectDir);
  const key = JSON.stringify([projectDir, sessionId, scope?.handle ?? null]);
  const generation = useRef(0);
  const operation = useRef(0);
  const [state, setState] = useState<{
    key: string;
    view: TaskViewObservation | null;
    list: TaskFileList | null;
    file: TaskFileText | null;
    directory: string;
    loading: boolean;
    error: string | null;
  }>({ key, view: null, list: null, file: null, directory: "", loading: true, error: null });
  const visible =
    state.key === key
      ? state
      : { key, view: null, list: null, file: null, directory: "", loading: true, error: null };
  useEffect(() => {
    const gen = ++generation.current;
    operation.current += 1;
    setState({
      key,
      view: null,
      list: null,
      file: null,
      directory: "",
      loading: Boolean(scope),
      error: scope ? null : "scope",
    });
    if (!scope) return;
    void (async () => {
      try {
        const view = await observeTaskView(scope.handle, sessionId, scope.project_dir);
        if (generation.current !== gen) return;
        const list = await listTaskFiles(scope.handle, view);
        if (generation.current !== gen) return;
        setState({ key, view, list, file: null, directory: "", loading: false, error: null });
      } catch {
        if (generation.current === gen)
          setState({
            key,
            view: null,
            list: null,
            file: null,
            directory: "",
            loading: false,
            error: "observe",
          });
      }
    })();
    return () => {
      generation.current += 1;
      operation.current += 1;
    };
  }, [key, scope, sessionId]);

  async function open(relativePath: string, isDir: boolean) {
    if (!scope || !visible.view || visible.loading) return;
    const gen = generation.current;
    const op = ++operation.current;
    const view = visible.view;
    setState((previous) => ({ ...previous, file: null, loading: true, error: null }));
    try {
      if (isDir) {
        const list = await listTaskFiles(scope.handle, view, relativePath);
        if (generation.current !== gen || operation.current !== op) return;
        setState({
          key,
          view,
          list,
          file: null,
          directory: relativePath,
          loading: false,
          error: null,
        });
      } else {
        const file = await readTaskFile(scope.handle, view, relativePath);
        if (generation.current !== gen || operation.current !== op) return;
        setState((previous) => ({ ...previous, file, loading: false, error: null }));
      }
    } catch {
      if (generation.current === gen && operation.current === op)
        setState((previous) => ({
          ...previous,
          file: null,
          loading: false,
          error: "read",
        }));
    }
  }

  return (
    <section role="region" aria-label={text("Read-only task files", "只读任务文件")} style={{ maxHeight: "45vh", overflow: "auto", flexShrink: 0 }}>
      <h2>{text("Read-only task files", "只读任务文件")}</h2>
      <button onClick={onClose}>{text("Close", "关闭")}</button>
      <p>
        {text("Authority project: ", "所属项目：")}
        {visible.view?.authority_project ?? projectDir}
      </p>
      <p>
        {text("Task: ", "任务：")}
        {sessionId}
      </p>
      <p>
        {text("Observed workspace: ", "查看的工作目录：")}
        {visible.view?.workspace_root ?? text("Unknown", "未知")}
      </p>
      <p>
        {text(
          "This viewer only observes files. It does not change the active IDE project.",
          "此查看器仅浏览文件，不会切换当前 IDE 项目。",
        )}
      </p>
      {visible.loading && <p role="status">{text("Reading task files…", "正在读取任务文件…")}</p>}
      {visible.error && (
        <p role="alert">
          {visible.error === "scope"
            ? text("No authorized project scope is available.", "当前没有可用的项目授权范围。")
            : visible.error === "observe"
              ? text(
                  "Could not observe or list this task workspace. Access may have changed.",
                  "无法查看或列出此任务工作目录，访问权限可能已变化。",
                )
              : text(
                  "Could not read this task file or directory. Binary, oversized, restricted files and changed bindings cannot be displayed.",
                  "无法读取此任务文件或目录。二进制、过大、受限文件或已变化的绑定无法显示。",
                )}
        </p>
      )}
      {visible.list && (
        <>
          <p>
            {text("Directory: ", "目录：")}
            {visible.directory || "/"}
          </p>
          {visible.directory && (
            <button
              disabled={visible.loading}
              onClick={() => void open(visible.directory.split("/").slice(0, -1).join("/"), true)}
            >
              {text("Parent directory", "上级目录")}
            </button>
          )}
          {visible.list.truncated && (
            <p role="status">
              {text(
                "This directory listing is truncated. Some entries are omitted.",
                "此目录列表已截断，部分条目未显示。",
              )}
            </p>
          )}
          {visible.list.entries.length === 0 && (
            <p>{text("No visible entries in this directory.", "此目录没有可见条目。")}</p>
          )}
          <ul>
            {visible.list.entries.map((entry) => (
              <li key={entry.relative_path}>
                <button
                  disabled={visible.loading}
                  onClick={() => void open(entry.relative_path, entry.is_dir)}
                >
                  {entry.is_dir ? text("Directory: ", "目录：") : text("File: ", "文件：")}
                  {entry.relative_path}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {visible.file && (
        <article>
          <h3>{visible.file.relative_path}</h3>
          <p>
            {visible.file.bytes} {text("bytes", "字节")}
          </p>
          <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{visible.file.content}</pre>
        </article>
      )}
    </section>
  );
}
