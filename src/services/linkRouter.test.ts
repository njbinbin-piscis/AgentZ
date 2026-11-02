import { describe, expect, it } from "vitest";
import { classifyLink, fileUrlToPath, opensWithSystem } from "./linkRouter";

describe("classifyLink", () => {
  it("sends web addresses to the built-in browser", () => {
    expect(classifyLink("https://example.com/a?b=1#c")).toEqual({ kind: "url", url: "https://example.com/a?b=1#c" });
    expect(classifyLink("http://localhost:3000")).toEqual({ kind: "url", url: "http://localhost:3000" });
  });

  it("treats mail/phone as external and leaves in-page anchors alone", () => {
    expect(classifyLink("mailto:a@b.c")).toEqual({ kind: "external", url: "mailto:a@b.c" });
    expect(classifyLink("#section")).toEqual({ kind: "ignore" });
    expect(classifyLink("")).toEqual({ kind: "ignore" });
  });

  it("refuses script and unknown schemes", () => {
    expect(classifyLink("javascript:alert(1)")).toEqual({ kind: "ignore" });
    expect(classifyLink("data:text/html,x")).toEqual({ kind: "ignore" });
  });

  it("turns file URLs and bare paths into file targets", () => {
    expect(classifyLink("file:///C:/my%20proj/a.md")).toEqual({ kind: "file", path: "C:/my proj/a.md" });
    expect(classifyLink("file:///home/u/a.md")).toEqual({ kind: "file", path: "/home/u/a.md" });
    expect(classifyLink("docs/readme.md#L10")).toEqual({ kind: "file", path: "docs/readme.md" });
    expect(classifyLink("C:\\work\\a.txt")).toEqual({ kind: "file", path: "C:\\work\\a.txt" });
    expect(fileUrlToPath("file://host/share/x.txt")).toBe("//host/share/x.txt");
  });
});

describe("opensWithSystem", () => {
  it("hands unsupported types, folders and (when not previewable) PDFs to the OS", () => {
    expect(opensWithSystem("a/report.docx", { pdfPreview: false })).toBe(true);
    expect(opensWithSystem("a/build.zip", { pdfPreview: true })).toBe(true);
    expect(opensWithSystem("a/dir/", { pdfPreview: true })).toBe(true);
    expect(opensWithSystem("a/doc.pdf", { pdfPreview: false })).toBe(true);
    expect(opensWithSystem("a/doc.pdf", { pdfPreview: true })).toBe(false);
    expect(opensWithSystem("src/main.ts", { pdfPreview: false })).toBe(false);
    expect(opensWithSystem("img.png", { pdfPreview: false })).toBe(false);
  });
});
