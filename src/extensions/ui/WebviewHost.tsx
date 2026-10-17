import { useEffect, useMemo, useRef } from "react";
import { extensionService } from "../extensionService";
import type { WebviewState } from "../extensionUiStore";
import { buildWebviewSrcDoc, isBoundedWebviewMessage } from "./webviewSecurity";

/**
 * Hosts an extension webview's HTML inside a sandboxed iframe. Messages flow:
 *   extension -> $postMessage -> "agentz-webview-post" event -> iframe
 *   iframe -> postMessage -> here -> extensionService.postWebviewMessage
 */
export default function WebviewHost({ webview }: { webview: WebviewState }) {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const onPost = (e: Event) => {
      const detail = (e as CustomEvent).detail as { handle?: unknown; message?: unknown } | null;
      if (detail?.handle !== webview.handle || !isBoundedWebviewMessage(detail.message)) return;
      iframeRef.current?.contentWindow?.postMessage({ __agentzWebview: true, message: detail.message }, "*");
    };
    window.addEventListener("agentz-webview-post", onPost);

    const onMessage = (e: MessageEvent) => {
      if (e.source !== iframeRef.current?.contentWindow || e.origin !== "null") return;
      if (!e.data || typeof e.data !== "object" || e.data.__agentzWebview !== true) return;
      if (!isBoundedWebviewMessage(e.data.message)) return;
      extensionService.postWebviewMessage(webview.handle, e.data.message);
    };
    window.addEventListener("message", onMessage);

    return () => {
      window.removeEventListener("agentz-webview-post", onPost);
      window.removeEventListener("message", onMessage);
    };
  }, [webview.handle]);

  const appNonce = typeof document === "undefined"
    ? undefined
    : document.querySelector<HTMLScriptElement>("script[nonce]")?.nonce
      ?? (import.meta.env.DEV ? "agentz-webview-dev-nonce" : undefined);
  const scriptsEnabled = webview.enableScripts && Boolean(appNonce);
  const srcDoc = useMemo(
    () => buildWebviewSrcDoc(webview.html, scriptsEnabled, appNonce, webview.resourceToken),
    [webview.html, scriptsEnabled, appNonce, webview.resourceToken],
  );

  return (
    <iframe
      ref={iframeRef}
      className="agentz-webview-frame"
      title={webview.title}
      sandbox={scriptsEnabled ? "allow-scripts" : ""}
      referrerPolicy="no-referrer"
      srcDoc={srcDoc}
    />
  );
}
