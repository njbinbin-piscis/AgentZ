import { Component, useEffect, useMemo, useState, type ErrorInfo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import CodeEditor, { type EditorConflictBannerProps } from "./CodeEditor";
import ImagePreview from "./ImagePreview";
import MarkdownPreview from "./MarkdownPreview";
import NotebookEditor from "../../extensions/notebook/NotebookEditor";
import { isHtmlPath, isMarkdownPath, isPreviewablePath } from "./FileIcon";
import type { OpenTab, TabViewMode } from "./types";
import "./MarkdownPreview.css";
import { buildHtmlPreviewSrcDoc } from "../../extensions/ui/webviewSecurity";

interface FileViewerProps {
  tab: OpenTab;
  projectDir: string | null;
  reveal?: { line: number; column: number; nonce: number } | null;
  onChange: (value: string) => void;
  onSave?: () => void;
  onViewModeChange: (mode: TabViewMode) => void;
  /** Load this file in the embedded browser over file:// (full JS/network). */
  onOpenInBrowser?: () => void;
  conflict?: EditorConflictBannerProps | null;
}

function HtmlPreview({ content }: { content: string }) {
  const { t } = useTranslation();
  const srcDoc = useMemo(() => {
    try {
      return buildHtmlPreviewSrcDoc(content);
    } catch (e) {
      console.error("[AgentZ] HTML preview failed", e);
      return "";
    }
  }, [content]);
  if (!srcDoc) return null;
  return (
    <div className="agentz-html-preview">
      <iframe
        title={t("ide.htmlPreview")}
        sandbox="allow-scripts allow-forms allow-modals"
        srcDoc={srcDoc}
      />
    </div>
  );
}

class PreviewErrorBoundary extends Component<
  { resetKey: string; onError: () => void; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[AgentZ] file preview failed", this.props.resetKey, error, info);
    this.props.onError();
  }

  componentDidUpdate(prev: { resetKey: string }) {
    if (this.state.failed && prev.resetKey !== this.props.resetKey) {
      this.setState({ failed: false });
    }
  }

  render() {
    if (this.state.failed) return null;
    return this.props.children;
  }
}

export default function FileViewer({
  tab,
  projectDir,
  reveal,
  onChange,
  onSave,
  onViewModeChange,
  onOpenInBrowser,
  conflict,
}: FileViewerProps) {
  const { t } = useTranslation();
  const [previewBroken, setPreviewBroken] = useState(false);
  const storedMode = tab.viewMode ?? "editor";
  const viewMode = storedMode === "preview" && previewBroken ? "editor" : storedMode;
  useEffect(() => {
    setPreviewBroken(false);
  }, [tab.path]);

  if (tab.isDiff && tab.originalContent !== undefined) {
    return (
      <CodeEditor
        tab={tab}
        theme="violet"
        projectDir={projectDir}
        reveal={reveal}
        onChange={onChange}
        onSave={onSave}
        conflict={conflict}
      />
    );
  }

  if (tab.path.toLowerCase().endsWith(".ipynb") && !tab.isReadOnly) {
    return (
      <div className="agentz-file-view-body">
        <PreviewErrorBoundary resetKey={tab.path} onError={() => setPreviewBroken(true)}>
          <NotebookEditor content={tab.content ?? ""} onChange={onChange} />
        </PreviewErrorBoundary>
        {previewBroken && <div className="agentz-preview-failed">{t("ide.previewFailed")}</div>}
      </div>
    );
  }

  const previewable = isPreviewablePath(tab.path);

  if (viewMode === "image" && tab.previewData) {
    return (
      <ImagePreview src={tab.previewData} name={tab.name} size={tab.fileSize} />
    );
  }

  return (
    <div className="agentz-file-view-body">
      {previewable && (
        <div className="agentz-file-view-toolbar">
          <button
            type="button"
            className={viewMode === "editor" ? "active" : ""}
            onClick={() => onViewModeChange("editor")}
          >
            {t("ide.viewSource")}
          </button>
          <button
            type="button"
            className={viewMode === "preview" ? "active" : ""}
            onClick={() => {
              setPreviewBroken(false);
              onViewModeChange("preview");
            }}
          >
            {t("ide.viewPreview")}
          </button>
          {onOpenInBrowser && isHtmlPath(tab.path) && !tab.isReadOnly && (
            <button type="button" onClick={onOpenInBrowser} title={t("ide.openInBrowserHint")}>
              {t("ide.openInBrowser")}
            </button>
          )}
        </div>
      )}
      {previewBroken && <div className="agentz-preview-failed">{t("ide.previewFailed")}</div>}

      {viewMode === "preview" && previewable ? (
        <PreviewErrorBoundary
          resetKey={tab.path}
          onError={() => {
            setPreviewBroken(true);
            onViewModeChange("editor");
          }}
        >
          {isMarkdownPath(tab.path) ? (
            <MarkdownPreview content={tab.content ?? ""} />
          ) : isHtmlPath(tab.path) ? (
            <HtmlPreview content={tab.content ?? ""} />
          ) : null}
        </PreviewErrorBoundary>
      ) : (
        <CodeEditor
          tab={tab}
          theme="violet"
          projectDir={projectDir}
          reveal={reveal}
          onChange={onChange}
          onSave={onSave}
          conflict={conflict}
        />
      )}
    </div>
  );
}
