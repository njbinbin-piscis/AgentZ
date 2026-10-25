import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "./ReviewPanel";

describe("parseUnifiedDiff", () => {
  it("tracks file paths and new-file line numbers", () => {
    const diff = [
      "diff --git a/src/a.ts b/src/a.ts",
      "index 1..2 100644",
      "--- a/src/a.ts",
      "+++ b/src/a.ts",
      "@@ -10,3 +10,4 @@ fn",
      " keep",
      "-old",
      "+new",
      "+added",
      " tail",
    ].join("\n");
    const lines = parseUnifiedDiff(diff);
    expect(lines[0]).toMatchObject({ kind: "file", path: "src/a.ts" });
    const body = lines.filter((l) => ["add", "remove", "context"].includes(l.kind));
    expect(body.map((l) => [l.kind, l.text, l.line])).toEqual([
      ["context", "keep", 10],
      ["remove", "old", undefined],
      ["add", "new", 11],
      ["add", "added", 12],
      ["context", "tail", 13],
    ]);
  });
});
