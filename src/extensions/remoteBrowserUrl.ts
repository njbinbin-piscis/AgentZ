/** Remote port inside a localhost URL, or null when the URL is not a loopback page. */
export function remoteLocalhostPort(url: string): number | null {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return null;
  if (parsed.hostname !== "localhost" && parsed.hostname !== "127.0.0.1") return null;
  const port = Number(parsed.port || (parsed.protocol === "https:" ? 443 : 80));
  return Number.isInteger(port) && port > 0 ? port : null;
}

/** Point a loopback URL at the local forward of that remote port. */
export function urlOnForward(url: string, localPort: number): string {
  const parsed = new URL(url);
  parsed.hostname = "127.0.0.1";
  parsed.port = String(localPort);
  return parsed.toString();
}
