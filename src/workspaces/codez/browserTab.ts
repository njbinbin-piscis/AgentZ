/** Synthetic editor tab path for the embedded browser panel. */
export const BROWSER_TAB_PATH = "__agentz_browser__";

export function isBrowserTab(path: string | null | undefined): boolean {
  return path === BROWSER_TAB_PATH;
}

/**
 * Absolute local path -> `file://` URL the embedded browser can load.
 * `C:\a b\x.html` -> `file:///C:/a%20b/x.html`; UNC `\\host\s\x` -> `file://host/s/x`.
 */
export function localFileUrl(absPath: string): string {
  const p = absPath.replace(/\\/g, "/");
  const enc = (s: string) => s.split("/").map((seg) => encodeURIComponent(seg).replace(/%3A/gi, ":")).join("/");
  if (p.startsWith("//")) return `file:${enc(p)}`;
  return `file://${enc(p.startsWith("/") ? p : `/${p}`)}`;
}