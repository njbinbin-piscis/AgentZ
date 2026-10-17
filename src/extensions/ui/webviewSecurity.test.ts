import { describe, expect, it } from "vitest";
import { buildWebviewSrcDoc, isBoundedWebviewMessage } from "./webviewSecurity";

const nonce = "0123456789abcdef0123456789abcdef";

describe("extension webview isolation policy", () => {
  it("places policy before deceptive document tags and early extension scripts", () => {
    for (const html of ["<!-- <head> --><script>run()</script>", '<script>const x="<head>"</script>', '<script>run()</script><html><head></head></html>']) {
      const doc = buildWebviewSrcDoc(html, true, nonce);
      expect(doc.startsWith('<html><head><meta http-equiv="Content-Security-Policy"')).toBe(true);
      expect(doc.indexOf("connect-src 'none'")).toBeLessThan(doc.indexOf("run()") < 0 ? doc.indexOf("const x") : doc.indexOf("run()"));
    }
  });

  it("does not mistake src text inside another attribute for a scoped script", () => {
    const token = "12345678-1234-4234-8234-123456789abc";
    const local = `agentz-resource://localhost/${token}/YWJj`;
    const doc = buildWebviewSrcDoc(`<script data-example=' src="${local}"' src="https://example.invalid/a.js" nonce="untrusted"></script>`, true, nonce, token);
    expect(doc).toContain(`<script data-example=' src="${local}"' src="https://example.invalid/a.js"></script>`);
    expect(doc.match(new RegExp(`nonce="${nonce}"`, "g"))).toHaveLength(1);
  });

  it("preserves nonce-shaped text in quoted attributes and removes actual boolean nonces", () => {
    const doc = buildWebviewSrcDoc('<script data-example=" nonce=sample" nonce>run()</script>', true, nonce);
    expect(doc).toContain(`<script data-example=" nonce=sample" nonce="${nonce}">run()</script>`);
  });

  it("keeps scripts disabled unless explicitly enabled and emits a restrictive policy", () => {
    const doc = buildWebviewSrcDoc("<h1>Preview</h1><script>run()</script>", false, nonce);
    expect(doc).toContain("default-src 'none'");
    expect(doc).toContain("script-src 'none'");
    expect(doc).not.toContain("nonce=\"0123456789abcdef0123456789abcdef\"");
    expect(doc).not.toContain("unsafe-inline");
    expect(doc).toContain("<h1>Preview</h1>");
  });

  it("does not inject a predictable nonce if the Tauri page exposes none", () => {
    const doc = buildWebviewSrcDoc("<style>.x{color:red}</style><script>run()</script>", true);
    expect(doc).toContain("script-src 'none'");
    expect(doc).toContain("style-src 'none'");
    expect(doc).not.toContain("nonce=\"");
    expect(doc).not.toContain("acquireVsCodeApi");
  });

  it("binds the API shim to a nonce and keeps panel state in that iframe", () => {
    const doc = buildWebviewSrcDoc("<style>.sample{color:red}</style><script nonce='untrusted-value'>acquireVsCodeApi().postMessage('ready')</script>", true, nonce);
    expect(doc).toContain(`script-src 'nonce-${nonce}'`);
    expect(doc).toContain(`<script nonce="${nonce}">`);
    expect(doc).toContain("let state=null");
    expect(doc).toContain(`<style nonce="${nonce}">`);
    expect(doc).toContain("acquireVsCodeApi may only be called once");
    expect(doc).not.toContain("localStorage");
    expect(doc).not.toContain("unsafe-inline");
  });

  it("allows only the scoped resource protocol for panel assets", () => {
    const doc = buildWebviewSrcDoc('<link rel="stylesheet" href="agentz-resource://token/style"><img src="agentz-resource://token/image">', false, nonce);
    expect(doc).toContain("style-src 'nonce-");
    expect(doc).toContain("agentz-resource:");
    expect(doc).toContain("http://agentz-resource.localhost");
    expect(doc).toContain("connect-src 'none'");
    expect(doc).toContain("script-src 'none'");
  });

  it("does not expand script authority to remote script tags", () => {
    const doc = buildWebviewSrcDoc('<script src="https://example.invalid/payload.js" nonce="untrusted-value"></script>', true, nonce);
    expect(doc).toContain("<script src=\"https://example.invalid/payload.js\"></script>");
    expect(doc.match(/nonce="0123456789abcdef0123456789abcdef"/g)).toHaveLength(1);
    expect(doc).toContain("connect-src 'none'");
    expect(doc).toContain("frame-src 'none'");
  });

  it("adds a nonce only to external scripts routed through this panel's resource token", () => {
    const token = "12345678-1234-4234-8234-123456789abc";
    const local = `agentz-resource://localhost/${token}/YWJj`;
    const otherPanel = "agentz-resource://localhost/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/YWJj";
    const doc = buildWebviewSrcDoc(
      `<script src="${local}"></script><script src="${otherPanel}"></script><script src="https://example.invalid/app.js"></script>`,
      true,
      nonce,
      token,
    );
    expect(doc).toContain(`<script src="${local}" nonce="${nonce}"></script>`);
    expect(doc).toContain(`<script src="${otherPanel}"></script>`);
    expect(doc).toContain('<script src="https://example.invalid/app.js"></script>');
    expect(doc.match(new RegExp(`nonce="${nonce}"`, "g"))).toHaveLength(2);
  });

  it("does not rewrite tag-shaped text inside script source", () => {
    const source = '<script>const example = "<script>not a tag</script>";</script>';
    const doc = buildWebviewSrcDoc(source, true, nonce);
    expect(doc).toContain(`<script nonce="${nonce}">const example = "<script>not a tag</script>";</script>`);
  });

  it("preserves the document doctype and inserts policy into an existing head", () => {
    const doc = buildWebviewSrcDoc("<!doctype html><html><head><title>Panel</title></head><body>ok</body></html>", false, nonce);
    expect(doc.startsWith("<!doctype html><html><head><meta")).toBe(true);
    expect(doc).toContain("<title>Panel</title>");
    expect(doc).toContain("<body>ok</body>");
  });

  it("does not rewrite tag-shaped text inside stylesheet source", () => {
    const doc = buildWebviewSrcDoc('<style>.x:after{content:"<script>"}</style>', false, nonce);
    expect(doc).toContain(`<style nonce="${nonce}">.x:after{content:"<script>"}</style>`);
  });

  it("rejects oversized, circular and non-serializable messages", () => {
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(isBoundedWebviewMessage({ ok: true })).toBe(true);
    expect(isBoundedWebviewMessage(undefined)).toBe(false);
    expect(isBoundedWebviewMessage(circular)).toBe(false);
    expect(isBoundedWebviewMessage({ text: "x".repeat(1_048_577) })).toBe(false);
  });
});
