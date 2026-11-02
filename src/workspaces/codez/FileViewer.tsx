import { useMemo } from "react";
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
  const srcDoc = useMemo(() => buildHtmlPreviewSrcDoc(content), [content]);
  return (
    <div className="agentz-html-preview">
      <iframe
        title="HTML preview"
        sandbox="allow-scripts allow-forms allow-modals"
        srcDoc={srcDoc}
      />
    </div>
  );
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
        <NotebookEditor content={tab.content} onChange={onChange} />
      </div>
    );
  }

  const previewable = isPreviewablePath(tab.path);
  const viewMode = tab.viewMode ?? "editor";

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
            onClick={() => onViewModeChange("preview")}
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

      {viewMode === "preview" && previewable ? (
        isMarkdownPath(tab.path) ? (
          <MarkdownPreview content={tab.content} />
        ) : isHtmlPath(tab.path) ? (
          <HtmlPreview content={tab.content} />
        ) : null
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
