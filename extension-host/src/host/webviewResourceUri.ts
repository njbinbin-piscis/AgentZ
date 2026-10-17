import path from "node:path";

export interface FileUriLike {
  toString(): string;
  scheme?: string;
  fsPath?: string;
}

export function normalizeWebviewResourcePath(filePath: string): string {
  if (process.platform === "win32" && /^\/[A-Za-z]:\//.test(filePath)) {
    filePath = filePath.slice(1);
  }
  return path.resolve(filePath);
}

export function asWebviewResourceUri(uri: FileUriLike, token: string): string {
  if (!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i.test(token)) {
    throw new Error("Invalid webview resource token");
  }
  if (uri.scheme && uri.scheme !== "file") {
    throw new Error("Webview.asWebviewUri only accepts file URIs");
  }
  let filePath = uri.fsPath;
  if (!filePath) {
    const parsed = new URL(uri.toString());
    if (parsed.protocol !== "file:") throw new Error("Webview.asWebviewUri only accepts file URIs");
    filePath = decodeURIComponent(parsed.pathname);
  }
  const encodedPath = Buffer.from(normalizeWebviewResourcePath(filePath), "utf8").toString("base64url");
  return `agentz-resource://localhost/${token}/${encodedPath}`;
}
