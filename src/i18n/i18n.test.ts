import { describe, expect, it } from "vitest";
import en from "./en";
import zh from "./zh";

const tsxModules = import.meta.glob("../**/*.tsx", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

function flatten(obj: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return flatten(value as Record<string, unknown>, path);
    }
    return [path];
  });
}

describe("i18n parity", () => {
  it("en and zh expose the same key paths", () => {
    const enKeys = new Set(flatten(en as Record<string, unknown>));
    const zhKeys = new Set(flatten(zh as Record<string, unknown>));
    expect([...enKeys].sort()).toEqual([...zhKeys].sort());
  });

  it("includes explorer expand/collapse tooltips", () => {
    expect(en.ide.expandAll).toBeTruthy();
    expect(zh.ide.expandAll).toBeTruthy();
    expect(en.ide.collapseAll).toBeTruthy();
    expect(zh.ide.collapseAll).toBeTruthy();
  });

  it("has zero hardcoded Chinese characters in production TSX files (R10-18)", () => {
    const cjkRe = /[\u4e00-\u9fff]/;
    const violations: string[] = [];
    for (const [file, content] of Object.entries(tsxModules)) {
      if (file.endsWith(".test.tsx")) continue;
      const lines = content.split(/\r?\n/);
      lines.forEach((line, idx) => {
        if (cjkRe.test(line)) {
          violations.push(`${file.replace(/^\.\.\//, "")}:${idx + 1}: ${line.trim()}`);
        }
      });
    }
    expect(violations).toEqual([]);
  });
});
