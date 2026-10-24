import { describe, expect, it } from "vitest";
import { collectArtifactsFromSteps, collectArtifactsFromToolSteps } from "./agentArtifacts";

describe("artifact collection", () => {
  it("ignores reads, listings and searches; keeps writes and edits", () => {
    const got = collectArtifactsFromToolSteps(
      [
        { name: "file_list", input: { path: "src/components" } },
        { name: "file_read", input: { path: "README.md" } },
        { name: "file_search", input: { path: "src" } },
        { name: "file_write", input: { path: "out/report.md" } },
        { name: "file_edit", input: { path: "src/a.ts" } },
      ],
      ["/repo"],
    );
    expect(got).toEqual(["src/a.ts", "out/report.md"]);
  });

  it("skips failed write tools in persisted steps", () => {
    const got = collectArtifactsFromSteps([
      {
        role: "assistant",
        text: "",
        tools: [
          { id: "1", name: "file_write", status: "error", input: { path: "x.md" } },
          { id: "2", name: "file_write", status: "done", input: { path: "y.md" } },
        ],
      },
    ]);
    expect(got).toEqual(["y.md"]);
  });
});
