import { describe, expect, it } from "vitest";
import path from "node:path";
import { asWebviewResourceUri, normalizeWebviewResourcePath } from "./webviewResourceUri";

const token = "12345678-1234-4234-8234-123456789abc";

describe("scoped extension resource URIs", () => {
  it("normalizes Windows file URI paths before encoding them", () => {
    const input = process.platform === "win32" ? "/C:/AgentZ/ext/media/icon.svg" : "/opt/AgentZ/ext/media/icon.svg";
    const expected = path.resolve(process.platform === "win32" ? "C:/AgentZ/ext/media/icon.svg" : input);
    const uri = asWebviewResourceUri({ scheme: "file", fsPath: input, toString: () => `file://${input}` }, token);
    const [scheme, authority, actualToken, encodedPath] = uri.split(/[/:]/).filter(Boolean);

    expect(scheme).toBe("agentz-resource");
    expect(authority).toBe("localhost");
    expect(actualToken).toBe(token);
    expect(Buffer.from(encodedPath, "base64url").toString("utf8")).toBe(expected);
  });

  it("refuses non-file URIs and invalid panel tokens", () => {
    expect(() => asWebviewResourceUri({ scheme: "https", fsPath: "/tmp/a.png", toString: () => "https://example.invalid/a.png" }, token)).toThrow(/file URIs/);
    expect(() => asWebviewResourceUri({ scheme: "file", fsPath: "/tmp/a.png", toString: () => "file:///tmp/a.png" }, "predictable")).toThrow(/token/);
  });

  it("normalizes VS Code style drive paths", () => {
    const input = process.platform === "win32" ? "/D:/extensions/sample/assets" : "/tmp/extensions/sample/assets";
    const expected = path.resolve(process.platform === "win32" ? "D:/extensions/sample/assets" : input);
    expect(normalizeWebviewResourcePath(input)).toBe(expected);
  });
});
