import { useEffect, useState, useSyncExternalStore } from "react";
import { extensionService } from "../extensionService";
import { extensionUiStore } from "../extensionUiStore";
import type { CustomEditorContribution } from "../customEditorRegistry";
import WebviewHost from "./WebviewHost";

/**
 * Resolves a custom editor for `filePath` and hosts its webview in the editor tab.
 */
export default function CustomEditorHost({
  filePath,
  contribution,
}: {
  filePath: string;
  contribution: CustomEditorContribution;
}) {
  const [handle, setHandle] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const ui = useSyncExternalStore(extensionUiStore.subscribe, extensionUiStore.getSnapshot);
  const webview =
    ui.webviews.find((w) => w.handle === handle) ??
    ui.webviews.find(
      (w) =>
        w.editorResource &&
        w.editorResource.replace(/\\/g, "/").toLowerCase() === filePath.replace(/\\/g, "/").toLowerCase(),
    );

  useEffect(() => {
    let cancelled = false;
    setError(null);
    setHandle(null);
    void (async () => {
      try {
        const h = await extensionService.resolveCustomEditor(
          contribution.viewType,
          filePath,
          contribution.displayName,
        );
        if (!cancelled) {
          if (!h) setError(`No provider registered for ${contribution.viewType}`);
          else setHandle(h);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [filePath, contribution.viewType, contribution.displayName]);

  if (error) {
    return <div className="agentz-preview-failed">{error}</div>;
  }
  if (!webview) {
    return <div className="agentz-preview-failed">Loading custom editor…</div>;
  }
  return (
    <div className="agentz-custom-editor-host" style={{ height: "100%", width: "100%" }}>
      <WebviewHost webview={webview} />
    </div>
  );
}
