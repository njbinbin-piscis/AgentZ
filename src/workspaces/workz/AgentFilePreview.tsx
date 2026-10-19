import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { ideApi } from "../../services/tauri/ide";
import { recordUserRecentEdit } from "../../services/tauri/chat";
import type { FileContent } from "../codez/types";
import Markdown from "../codez/Markdown";
import ImagePreview from "../codez/ImagePreview";
import { isHtmlPath, isImagePath, isMarkdownPath } from "../codez/FileIcon";
import { resolveArtifactFullPath, isPdfPath } from "./artifactPaths";

interface AgentFilePreviewProps {
  projectDir: string;
  path: string;
  /** Isolated worktree the agent ran in (paths resolve here when set). */
  workspaceDir?: string | null;
  onClose: () => void;
  /** Handoff the isolated worktree + file into CodeZ editor. */
  onOpenInCodeZ?: (opts: { worktreePath: string; filePath: string }) => void;
  /** Continue the conversation with the user's latest edits to this file. */
  onContinueWithEdits?: (opts: { path: string; content: string }) => void;
}

type ViewMode = "preview" | "source" | "edit" | "diff";

function languageForPath(path: string, lang: string | null | undefined): string {
  if (lang?.trim()) return lang;
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    ts: "typescript",
    tsx: "typescriptreact",
    js: "javascript",
    jsx: "javascriptreact",
    rs: "rust",
    py: "python",
    go: "go",
    json: "json",
    yaml: "yaml",
    yml: "yaml",
    md: "markdown",
    html: "html",
    css: "css",
    sh: "bash",
    sql: "sql",
  };
  return map[ext] ?? (ext || "text");
}

function HtmlPreviewPane({ content }: { content: string }) {
  return (
    <div className="agentz-workz-preview-html">
      <iframe title="HTML preview" sandbox="allow-same-origin" srcDoc={content} />
    </div>
  );
}

function PdfPreviewPane({ dataUrl }: { dataUrl: string }) {
  return (
    <div className="agentz-workz-preview-pdf">
      <iframe title="PDF preview" src={dataUrl} />
    </div>
  );
}

function CodePreviewPane({ content, language }: { content: string; language: string }) {
  const md = useMemo(
    () => `\`\`\`${language}\n${content.replace(/```/g, "``\\`")}\n\`\`\``,
    [content, language],
  );
  return (
    <div className="agentz-workz-preview-code-wrap">
      <Markdown content={md} />
    </div>
  );
}

export default function AgentFilePreview({
  projectDir,
  path,
  workspaceDir,
  onClose,
  onOpenInCodeZ,
  onContinueWithEdits,
}: AgentFilePreviewProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [file, setFile] = useState<FileContent | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("preview");
  const [diffOriginal, setDiffOriginal] = useState("");
  const [diffModified, setDiffModified] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [saving, setSaving] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);

  const fullPath = resolveArtifactFullPath(projectDir, path, workspaceDir);
  const fileName = path.split("/").pop() ?? path;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setFile(null);
    setViewMode("preview");
    setDraftContent("");
    setSavedFlash(false);

    (async () => {
      try {
        const [read, diff] = await Promise.all([
          ideApi.readFile(fullPath),
          ideApi.gitDiff(projectDir, path).catch(() => null),
        ]);
        if (cancelled) return;
        setFile(read);
        setDraftContent(read.content ?? "");
        if (diff && (diff.original !== diff.modified || diff.hunks.length > 0)) {
          setDiffOriginal(diff.original);
          setDiffModified(diff.modified);
        } else {
          setDiffOriginal("");
          setDiffModified("");
        }
      } catch (e) {
        if (!cancelled) {
          const msg = String(e);
          setError(msg.includes("directory") ? t("agent.previewDirectory") : msg);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fullPath, projectDir, path]);

  const isEditableText = Boolean(file && !file.is_binary && !isImagePath(path) && !isPdfPath(path));
  const isDirty = Boolean(file && isEditableText && draftContent !== file.content);
  const effectiveDiffOriginal = diffOriginal || (file?.content ?? "");
  const effectiveDiffModified = isDirty ? draftContent : diffModified;
  const hasDiff =
    (diffOriginal !== diffModified && (diffOriginal.length > 0 || diffModified.length > 0)) ||
    isDirty;
  const previewableText = file && !file.is_binary && (isMarkdownPath(path) || isHtmlPath(path));
  const showImage = Boolean(file?.preview_data && isImagePath(path));
  const showPdf = Boolean(file?.preview_data && isPdfPath(path));

  const handleSave = useCallback(async (): Promise<boolean> => {
    if (!file || !isEditableText || saving) return false;
    setSaving(true);
    setError(null);
    try {
      const beforeContent = file.content;
      const digest = await ideApi.writeFile(fullPath, draftContent, file.sha256 ?? undefined);
      recordUserRecentEdit({
        projectDir: workspaceDir || projectDir,
        path,
        beforeContent,
        afterContent: draftContent,
      });
      if (workspaceDir && workspaceDir !== projectDir) {
        recordUserRecentEdit({
          projectDir,
          path,
          beforeContent,
          afterContent: draftContent,
        });
      }
      setFile((prev) =>
        prev ? { ...prev, content: draftContent, sha256: digest ?? prev.sha256 } : prev,
      );
      setDiffModified(draftContent);
      setSavedFlash(true);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setSaving(false);
    }
  }, [file, isEditableText, saving, fullPath, draftContent, workspaceDir, projectDir, path]);

  const handleContinueWithEdits = useCallback(async () => {
    if (isDirty) {
      const ok = await handleSave();
      if (!ok) return;
    }
    onContinueWithEdits?.({ path, content: draftContent });
  }, [isDirty, handleSave, onContinueWithEdits, path, draftContent]);

  const toolbar = (
    <div className="agentz-workz-preview-actions">
      {isEditableText && (
        <>
          <button
            type="button"
            data-testid="preview-mode-preview"
            className={viewMode === "preview" || viewMode === "source" ? "active" : ""}
            onClick={() => setViewMode(previewableText ? "preview" : "source")}
          >
            {previewableText ? t("ide.viewPreview") : t("agent.previewFile")}
          </button>
          {previewableText && (
            <button
              type="button"
              data-testid="preview-mode-source"
              className={viewMode === "source" ? "active" : ""}
              onClick={() => setViewMode("source")}
            >
              {t("ide.viewSource")}
            </button>
          )}
          <button
            type="button"
            data-testid="preview-mode-edit"
            className={viewMode === "edit" ? "active" : ""}
            onClick={() => setViewMode("edit")}
          >
            {t("agent.previewEdit")}
          </button>
        </>
      )}
      {hasDiff && (
        <button
          type="button"
          data-testid="preview-mode-diff"
          className={viewMode === "diff" ? "active" : ""}
          onClick={() => setViewMode("diff")}
        >
          {t("agent.previewDiff")}
        </button>
      )}
      {isEditableText && (viewMode === "edit" || isDirty) && (
        <button
          type="button"
          data-testid="preview-save-btn"
          className="agentz-workz-preview-save"
          disabled={saving || !isDirty}
          onClick={() => void handleSave()}
        >
          {saving
            ? t("agent.previewSaving")
            : savedFlash && !isDirty
              ? t("agent.previewSaved")
              : t("agent.previewSave")}
        </button>
      )}
      {onOpenInCodeZ && (
        <button
          type="button"
          className="agentz-workz-preview-open-codez"
          data-testid="preview-open-in-codez"
          onClick={() =>
            onOpenInCodeZ({
              worktreePath: workspaceDir || projectDir,
              filePath: path,
            })
          }
        >
          {t("agent.openInCodeZ")}
        </button>
      )}
      <button type="button" className="agentz-workz-preview-close" onClick={onClose} title={t("common.close")}>
        ×
      </button>
    </div>
  );

  let body: ReactNode = null;
  if (loading) {
    body = <div className="agentz-workz-preview-muted">{t("common.loading")}</div>;
  } else if (error) {
    body = <div className="agentz-workz-preview-error">{error}</div>;
  } else if (file && viewMode === "edit" && isEditableText) {
    body = (
      <textarea
        data-testid="preview-edit-textarea"
        className="agentz-workz-preview-editor"
        value={draftContent}
        onChange={(e) => {
          setDraftContent(e.target.value);
          setSavedFlash(false);
        }}
        onKeyDown={(e) => {
          if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
            e.preventDefault();
            void handleSave();
          }
        }}
        spellCheck={false}
      />
    );
  } else if (file && viewMode === "diff" && hasDiff) {
    body = (
      <pre className="agentz-workz-preview-diff" data-testid="preview-diff-pane">
        <del>{effectiveDiffOriginal}</del>
        {"\n---\n"}
        <ins>{effectiveDiffModified}</ins>
      </pre>
    );
  } else if (file?.preview_data && showImage) {
    body = <ImagePreview src={file.preview_data} name={fileName} size={file.size} />;
  } else if (file?.preview_data && showPdf) {
    body = <PdfPreviewPane dataUrl={file.preview_data} />;
  } else if (file?.is_binary) {
    body = (
      <div className="agentz-workz-preview-muted">
        {t("agent.previewBinary")} ({file.size} B)
      </div>
    );
  } else if (file) {
    const lang = languageForPath(path, file.language);
    if (isMarkdownPath(path) && viewMode === "preview") {
      body = (
        <div className="agentz-workz-preview-md">
          <Markdown content={file.content} />
        </div>
      );
    } else if (isHtmlPath(path) && viewMode === "preview") {
      body = <HtmlPreviewPane content={file.content} />;
    } else if (isMarkdownPath(path) || isHtmlPath(path)) {
      body = <CodePreviewPane content={file.content} language={lang} />;
    } else {
      body = <CodePreviewPane content={file.content} language={lang} />;
    }
  }

  return (
    <aside className="agentz-workz-preview">
      <div className="agentz-workz-preview-head">
        <span className="agentz-workz-preview-title" title={path}>
          {fileName}
          {isDirty ? " ●" : ""}
        </span>
        {toolbar}
      </div>
      <div className="agentz-workz-preview-body">{body}</div>
      {isEditableText && (viewMode === "edit" || savedFlash || isDirty) && onContinueWithEdits && (
        <div className="agentz-workz-preview-foot">
          <button
            type="button"
            data-testid="preview-continue-with-edits"
            className="agentz-workz-preview-continue-btn"
            disabled={saving}
            onClick={() => void handleContinueWithEdits()}
          >
            {t("agent.continueWithEdits")}
          </button>
        </div>
      )}
    </aside>
  );
}
