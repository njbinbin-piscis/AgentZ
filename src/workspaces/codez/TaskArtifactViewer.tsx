import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectScope } from "../../services/tauri/projectScope";
import MarkdownPreview from "./MarkdownPreview";
import { observeTaskView, type TaskViewObservation } from "../../services/tauri/taskView";
import {
  listTaskArtifacts,
  captureTaskArtifact,
  readTaskArtifact,
  deleteTaskArtifact,
  saveTaskArtifactText,
  type ArtifactKind,
  type ArtifactMetadata,
  type ArtifactList,
  type ArtifactText,
} from "../../services/tauri/taskArtifacts";

export default function TaskArtifactViewer({
  projectDir,
  sessionId,
  onClose,
  onContinue,
}: {
  projectDir: string;
  sessionId: string;
  onClose(): void;
  onContinue?: (version: ArtifactText) => boolean;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => (zh ? cn : en);
  const scope = useProjectScope(projectDir);
  const key = JSON.stringify([projectDir, sessionId, scope?.handle ?? null]);
  const generation = useRef(0);
  const operationEpoch = useRef(0);
  const inFlight = useRef<number | null>(null);
  const reconcile = useRef<string | null>(null);
  const [reconcileKey, setReconcileKey] = useState<string | null>(null);
  const requiresReconcile = reconcileKey === key;
  const [state, setState] = useState<{
    key: string;
    view: TaskViewObservation | null;
    listing: ArtifactList | null;
    selected: ArtifactText | null;
    pending: ArtifactMetadata | null;
    busy: boolean;
    message: string | null;
  }>({ key, view: null, listing: null, selected: null, pending: null, busy: true, message: null });
  const [input, setInput] = useState({
    relativePath: "",
    title: "",
    kind: "report" as ArtifactKind,
  });
  const [draft, setDraft] = useState("");
  const [preview, setPreview] = useState(false);
  const visible =
    state.key === key
      ? state
      : {
          key,
          view: null,
          listing: null,
          selected: null,
          pending: null,
          busy: true,
          message: null,
        };
  useEffect(() => {
    const gen = ++generation.current;
    operationEpoch.current += 1;
    inFlight.current = null;
    reconcile.current = null;
    setReconcileKey(null);
    setInput({ relativePath: "", title: "", kind: "report" });
    setDraft("");
    setPreview(false);
    setState({
      key,
      view: null,
      listing: null,
      selected: null,
      pending: null,
      busy: !!scope,
      message: scope ? null : "scope",
    });
    if (!scope) return;
    void (async () => {
      try {
        const view = await observeTaskView(scope.handle, sessionId, scope.project_dir);
        if (generation.current !== gen) return;
        const listing = await listTaskArtifacts(scope.handle, view, null, 200);
        if (generation.current !== gen) return;
        setState({ key, view, listing, selected: null, pending: null, busy: false, message: null });
      } catch {
        if (generation.current === gen)
          setState({
            key,
            view: null,
            listing: null,
            selected: null,
            pending: null,
            busy: false,
            message: "error",
          });
      }
    })();
    return () => {
      generation.current += 1;
    };
  }, [key, scope, sessionId]);

  async function action(
    kind: "read" | "capture" | "delete" | "next" | "first" | "save" | "restore" | "continue",
    metadata?: ArtifactMetadata,
  ) {
    const mutation = kind === "capture" || kind === "delete" || kind === "save" || kind === "restore";
    if (
      !scope ||
      visible.busy ||
      inFlight.current !== null ||
      (kind !== "first" && !visible.view) ||
      (mutation && reconcile.current === key)
    )
      return;
    const gen = generation.current;
    const epoch = ++operationEpoch.current;
    inFlight.current = epoch;
    let view = visible.view;
    const selectedBefore = visible.selected;
    const listingBefore = visible.listing;
    setState((old) => ({ ...old, busy: true, message: null, selected: null, pending: null }));
    try {
      if (kind === "first") {
        view = await observeTaskView(scope.handle, sessionId, scope.project_dir);
        if (generation.current !== gen) return;
      }
      if (!view) throw new Error("No fresh task observation");
      let message: string | null = null;
      if (kind === "continue" && selectedBefore && onContinue) {
        const fresh = await observeTaskView(scope.handle, sessionId, scope.project_dir);
        if (generation.current !== gen) return;
        const version = await readTaskArtifact(scope.handle, fresh, selectedBefore.metadata.artifact_id, selectedBefore.metadata.version);
        if (generation.current !== gen) return;
        if (!version || version.metadata.payload_sha256 !== selectedBefore.metadata.payload_sha256) throw new Error("Artifact version changed");
        if (!onContinue(version)) throw new Error("Task changed before draft insertion");
        setState(old => ({ ...old, view: fresh, selected: version, busy: false, message: "attached" }));
        return;
      }
      if (kind === "read" && metadata) {
        const selected = await readTaskArtifact(
          scope.handle,
          view,
          metadata.artifact_id,
          metadata.version,
        );
        if (generation.current !== gen) return;
        setDraft(selected?.content ?? "");
        setPreview(false);
        setState((old) => ({
          ...old,
          selected,
          busy: false,
          message: selected ? null : "not_found",
        }));
        return;
      }
      if (kind === "capture") {
        await captureTaskArtifact(scope.handle, view, input);
        message = "captured";
      }
      if (kind === "save" || kind === "restore") {
        if (!selectedBefore || !listingBefore || listingBefore.page.has_more) throw new Error("Refresh all versions before editing");
        const siblings = listingBefore.page.versions.filter(m => m.artifact_id === selectedBefore.metadata.artifact_id);
        const latest = siblings[siblings.length - 1];
        if (!latest || (latest.kind !== "document" && latest.kind !== "report")) throw new Error("No editable head");
        const content = kind === "restore" ? selectedBefore.content : draft;
        const saved = await saveTaskArtifactText(scope.handle, view, latest, content);
        message = saved.version === latest.version ? "unchanged" : "revised";
      }
      if (kind === "delete" && metadata)
        message = await deleteTaskArtifact(
          scope.handle,
          view,
          metadata.artifact_id,
          metadata.version,
        );
      if (generation.current !== gen) return;
      const after = kind === "next" ? (visible.listing?.page.next_cursor ?? null) : null;
      const listing = await listTaskArtifacts(scope.handle, view, after, 200);
      if (generation.current !== gen) return;
      if (kind === "first") {
        reconcile.current = null;
        setReconcileKey(null);
      }
      setState((old) => ({ ...old, view, listing, busy: false, message }));
    } catch {
      if (generation.current === gen) {
        if (mutation) {
          reconcile.current = key;
          setReconcileKey(key);
        }
        setState((old) => ({
          ...old,
          busy: false,
          selected: null,
          pending: null,
          message: "error",
        }));
      }
    } finally {
      if (inFlight.current === epoch) inFlight.current = null;
    }
  }
  const messages: Record<string, string> = {
    scope: text("No authorized project scope.", "没有可用的项目授权范围。"),
    error: text(
      "The operation could not be verified. No success is assumed; refresh before retrying.",
      "无法确认操作结果，不视为成功；请刷新后再重试。",
    ),
    captured: text("A report version was saved.", "已保存产物版本。"),
    revised: text("A new text version was saved. Refresh before continuing the task.", "已保存新的文本版本；续聊前请刷新。"),
    unchanged: text("Text matches the current version; no new version was needed.", "文本与当前版本一致，无需新建版本。"),
    attached: text("Version text was added to the next message draft.", "已将版本文本加入下一条消息草稿。"),
    deleted: text(
      "Version payload deleted; live workspace file unchanged.",
      "已删除版本内容；工作目录文件未改动。",
    ),
    already_deleted: text("This version was already deleted.", "此版本此前已删除。"),
    not_found: text("This version was not found.", "未找到此版本。"),
  };
  return (
    <section
      role="region"
      aria-label={text("Task artifact versions", "任务产物版本")}
      style={{ maxHeight: "45vh", overflow: "auto" }}
    >
      <h2>{text("Task artifact versions", "任务产物版本")}</h2>
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
        {text("Workspace: ", "工作目录：")}
        {visible.view?.workspace_root ?? text("Unknown", "未知")}
      </p>
      <p>
        {text(
          "Stored text snapshots only. Webpages are shown as source; ChangeSet labels do not prove a complete Git review.",
          "仅查看已存文本快照。网页显示源码；ChangeSet 标签不代表完整 Git 评审。",
        )}
      </p>
      <label>
        {text("Relative file path", "相对文件路径")}
        <input
          value={input.relativePath}
          disabled={visible.busy}
          onChange={(e) => setInput((old) => ({ ...old, relativePath: e.target.value }))}
        />
      </label>
      <label>
        {text("Title", "标题")}
        <input
          value={input.title}
          disabled={visible.busy}
          onChange={(e) => setInput((old) => ({ ...old, title: e.target.value }))}
        />
      </label>
      <label>
        {text("Kind", "类型")}
        <select
          value={input.kind}
          disabled={visible.busy}
          onChange={(e) => setInput((old) => ({ ...old, kind: e.target.value as ArtifactKind }))}
        >
          {(["document", "report", "webpage", "validation", "change_set"] as ArtifactKind[]).map(
            (kind) => (
              <option key={kind} value={kind}>
                {
                  {
                    document: text("Document", "文档"),
                    report: text("Report", "报告"),
                    webpage: text("Webpage", "网页"),
                    validation: text("Validation", "验证"),
                    change_set: text("ChangeSet label", "变更集标签"),
                  }[kind]
                }
              </option>
            ),
          )}
        </select>
      </label>
      {input.kind === "report" && input.relativePath.toLowerCase().endsWith(".json") && (
        <p>{text(
          "Structured report JSON is checked against this task and rendered as a Markdown report before capture.",
          "结构化报告 JSON 会按当前任务校验，并在收录前渲染为 Markdown 报告。",
        )}</p>
      )}
      <button
        disabled={
          visible.busy ||
          requiresReconcile ||
          !visible.view ||
          !input.relativePath.trim() ||
          !input.title.trim()
        }
        onClick={() => void action("capture")}
      >
        {text("Save explicit version", "保存指定版本")}
      </button>
      <button disabled={visible.busy || !scope} onClick={() => void action("first")}>
        {text("Refresh versions", "刷新版本")}
      </button>
      {visible.busy && (
        <p role="status">{text("Reading or saving versions…", "正在读取或保存版本…")}</p>
      )}
      {visible.message && (
        <p role={visible.message === "error" ? "alert" : "status"}>{messages[visible.message]}</p>
      )}
      {visible.listing?.store_missing && (
        <p>{text("No artifact store exists for this project yet.", "此项目尚无产物库。")}</p>
      )}
      {visible.listing && (
        <>
          <p>{text("This page contains up to 200 versions.", "此页最多显示 200 个版本。")}</p>
          <ul>
            {visible.listing.page.versions.map((m) => (
              <li key={`${m.artifact_id}:${m.version}`}>
                <span>
                  {m.title} · v{m.version} · {m.bytes} {text("bytes", "字节")}
                </span>
                <button disabled={visible.busy} onClick={() => void action("read", m)}>
                  {text("View text", "查看文本")}
                </button>
                <button
                  disabled={visible.busy || requiresReconcile}
                  onClick={() => setState((old) => ({ ...old, pending: m }))}
                >
                  {text("Delete version", "删除版本")}
                </button>
              </li>
            ))}
          </ul>
          {visible.listing.page.has_more && (
            <button disabled={visible.busy} onClick={() => void action("next")}>
              {text("Next page (more versions exist)", "下一页（还有更多版本）")}
            </button>
          )}
          {!visible.listing.store_missing && visible.listing.page.versions.length === 0 && (
            <p>{text("No versions on this page.", "此页没有版本。")}</p>
          )}
        </>
      )}
      {visible.pending && (
        <div>
          <p>
            {text(
              "Delete stored payload for this version? This does not delete the live file.",
              "删除此版本的已存内容？此操作不会删除工作目录文件。",
            )}
            {visible.pending.title} · v{visible.pending.version}
          </p>
          <button
            disabled={visible.busy || requiresReconcile}
            onClick={() => void action("delete", visible.pending!)}
          >
            {text("Confirm deletion", "确认删除")}
          </button>
          <button onClick={() => setState((old) => ({ ...old, pending: null }))}>
            {text("Cancel", "取消")}
          </button>
        </div>
      )}
      {visible.selected && (
        <article>
          <h3>
            {visible.selected.metadata.title} · v{visible.selected.metadata.version}
          </h3>
          <p>SHA256: {visible.selected.metadata.payload_sha256}</p>
          <pre>{visible.selected.content}</pre>
          {onContinue && <button disabled={visible.busy || !visible.view} onClick={() => void action("continue")}>
            {text("Use this version in next message", "在下一条消息中使用此版本")}
          </button>}
          {(visible.selected.metadata.kind === "document" || visible.selected.metadata.kind === "report") && (
            <>
              <label>
                {text("Edit text", "编辑文本")}
                <textarea value={draft} disabled={visible.busy} onChange={e => setDraft(e.target.value)} rows={12} style={{ width: "100%" }} />
              </label>
              <button disabled={visible.busy} onClick={() => setPreview(value => !value)}>
                {preview ? text("Hide draft preview", "隐藏草稿预览") : text("Preview draft", "预览草稿")}
              </button>
              {preview && (visible.selected.metadata.source_relative_path?.toLowerCase().endsWith(".html") ? (
                <iframe title={text("Draft HTML preview", "HTML 草稿预览")} sandbox="" srcDoc={draft} />
              ) : (
                <MarkdownPreview content={draft} />
              ))}
              <button disabled={visible.busy || requiresReconcile || !visible.listing || visible.listing.page.has_more || !draft || new TextEncoder().encode(draft).length > 1048576} onClick={() => void action("save")}>
                {text("Save as new version", "另存为新版本")}
              </button>
              <button disabled={visible.busy || requiresReconcile || !visible.listing || visible.listing.page.has_more} onClick={() => void action("restore")}>
                {text("Restore this version as new", "将此版本恢复为新版本")}
              </button>
            </>
          )}
        </article>
      )}
    </section>
  );
}
