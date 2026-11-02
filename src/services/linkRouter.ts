import { invoke } from "@tauri-apps/api/core";

/**
 * Where a clicked link goes. The webview must never navigate to it itself
 * (that replaces the whole UI), so every anchor click is classified here and
 * routed: web pages to the built-in browser, project files to the previewer,
 * everything else to the operating system.
 */
export type LinkTarget =
  | { kind: "url"; url: string }
  | { kind: "file"; path: string }
  | { kind: "external"; url: string }
  | { kind: "ignore" };

const WINDOWS_DRIVE = /^[A-Za-z]:[\\/]/;

function stripQueryAndHash(path: string): string {
  const cut = path.search(/[?#]/);
  return cut >= 0 ? path.slice(0, cut) : path;
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function fileUrlToPath(href: string): string | null {
  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return null;
  }
  if (url.protocol !== "file:") return null;
  let path = safeDecode(url.pathname);
  if (/^\/[A-Za-z]:/.test(path)) path = path.slice(1);
  else if (url.host) path = `//${url.host}${path}`;
  return path || null;
}

export function classifyLink(rawHref: string | null | undefined): LinkTarget {
  const href = (rawHref ?? "").trim();
  if (!href || href.startsWith("#")) return { kind: "ignore" };
  if (/^https?:\/\//i.test(href)) return { kind: "url", url: href };
  if (/^(mailto|tel):/i.test(href)) return { kind: "external", url: href };
  if (/^file:/i.test(href)) {
    const path = fileUrlToPath(href);
    return path ? { kind: "file", path } : { kind: "ignore" };
  }
  // Any other scheme (javascript:, data:, custom) is not something to open.
  if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(href) && !WINDOWS_DRIVE.test(href)) return { kind: "ignore" };
  const path = safeDecode(stripQueryAndHash(href));
  return path ? { kind: "file", path } : { kind: "ignore" };
}

/** Types the in-app viewers cannot show; the system default program opens them. */
const SYSTEM_OPEN_EXT = new Set([
  "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "pages", "numbers", "key",
  "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "iso",
  "exe", "msi", "dmg", "pkg", "app", "deb", "rpm", "apk", "jar",
  "mp3", "wav", "flac", "ogg", "m4a", "aac", "mp4", "mkv", "avi", "mov", "webm", "wmv", "flv",
  "psd", "ai", "sketch", "fig", "dwg",
]);

export function opensWithSystem(path: string, opts: { pdfPreview: boolean }): boolean {
  const clean = stripQueryAndHash(path);
  if (/[\\/]$/.test(clean)) return true; // a folder
  const ext = clean.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase() ?? "";
  if (ext === "pdf") return !opts.pdfPreview;
  return SYSTEM_OPEN_EXT.has(ext);
}

export function openExternal(url: string): Promise<void> {
  return invoke<void>("plugin:shell|open", { path: url });
}

export const OPEN_LINK_EVENT = "agentz-open-link";

export interface OpenLinkDetail {
  /** Workspace that was visible when the link was clicked. */
  mode: string;
  target: LinkTarget;
}

/**
 * Capture every anchor click in the app UI. Without this the webview follows
 * `href` itself and the whole window turns into that page.
 */
export function installLinkInterceptor(route: (target: LinkTarget) => void): () => void {
  const onClick = (e: MouseEvent) => {
    if (e.button !== 0 && e.type === "click") return;
    const anchor = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    // In-page anchors keep their normal behaviour.
    if (href?.trim().startsWith("#")) return;
    e.preventDefault();
    e.stopPropagation();
    route(classifyLink(href));
  };
  document.addEventListener("click", onClick, true);
  document.addEventListener("auxclick", onClick, true);
  return () => {
    document.removeEventListener("click", onClick, true);
    document.removeEventListener("auxclick", onClick, true);
  };
}
