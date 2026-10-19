import { describe, expect, it } from "vitest";
import {
  filterArtifactPaths,
  isAbsoluteArtifactPath,
  relativizeArtifactPath,
  resolveArtifactFullPath,
  sortArtifactPaths,
} from "./artifactPaths";
import { collectArtifactsFromSteps, normalizeArtifactPath } from "./agentArtifacts";

describe("resolveArtifactFullPath", () => {
  it("uses an absolute Windows artifact path as-is instead of double-joining the root", () => {
    // Regression: File not found: D:\projects\AgentZ/D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md
    expect(
      resolveArtifactFullPath(
        "D:\\projects\\AgentZ",
        "D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md",
      ),
    ).toBe("D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md");
    expect(
      resolveArtifactFullPath(
        "D:\\projects\\AgentZ",
        "D:\\projects\\AgentZ\\MODULE_DEPENDENCY_GRAPH.md",
      ),
    ).toBe("D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md");
  });

  it("recovers a path that was already double-joined", () => {
    expect(
      resolveArtifactFullPath(
        "D:\\projects\\AgentZ",
        "D:/projects/AgentZ/D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md",
      ),
    ).toBe("D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md");
  });

  it("joins plain relative paths onto the workspace root when set", () => {
    expect(resolveArtifactFullPath("/repo", "docs/plan.md")).toBe("/repo/docs/plan.md");
    expect(resolveArtifactFullPath("/repo", "docs/plan.md", "/repo/.worktrees/wt-1")).toBe(
      "/repo/.worktrees/wt-1/docs/plan.md",
    );
  });

  it("does not double the root when the path already starts with it", () => {
    expect(resolveArtifactFullPath("/repo", "/repo/docs/plan.md")).toBe("/repo/docs/plan.md");
  });
});

describe("relativizeArtifactPath / normalizeArtifactPath", () => {
  it("relativizes absolute paths under a known root (case-insensitive drive)", () => {
    expect(
      normalizeArtifactPath("D:\\projects\\AgentZ\\docs\\plan.md", ["D:/Projects/AgentZ"]),
    ).toBe("docs/plan.md");
    expect(relativizeArtifactPath("/repo/src/a.ts", ["/repo"])).toBe("src/a.ts");
  });

  it("keeps absolute paths outside every root absolute", () => {
    expect(normalizeArtifactPath("E:/elsewhere/file.md", ["D:/projects/AgentZ"])).toBe(
      "E:/elsewhere/file.md",
    );
    expect(isAbsoluteArtifactPath("E:/elsewhere/file.md")).toBe(true);
  });

  it("cleans relative paths", () => {
    expect(normalizeArtifactPath("docs\\plan.md")).toBe("docs/plan.md");
    expect(normalizeArtifactPath("/docs/plan.md")).toBe("docs/plan.md");
  });
});

describe("collectArtifactsFromSteps ordering", () => {
  it("returns newest first and bumps re-touched files to the front", () => {
    const steps = [
      {
        role: "assistant" as const,
        text: "",
        tools: [
          { id: "1", name: "file_write", status: "done" as const, path: "a/first.md" },
          { id: "2", name: "file_write", status: "done" as const, path: "b/second.md" },
        ],
      },
      {
        role: "assistant" as const,
        text: "",
        tools: [
          { id: "3", name: "file_write", status: "done" as const, path: "c/third.md" },
          { id: "4", name: "file_edit", status: "done" as const, path: "a/first.md" },
        ],
      },
    ];
    expect(collectArtifactsFromSteps(steps)).toEqual([
      "a/first.md",
      "c/third.md",
      "b/second.md",
    ]);
  });

  it("relativizes absolute tool paths against the project root", () => {
    const steps = [
      {
        role: "assistant" as const,
        text: "",
        tools: [
          {
            id: "1",
            name: "file_write",
            status: "done" as const,
            path: "D:\\projects\\AgentZ\\MODULE_DEPENDENCY_GRAPH.md",
          },
        ],
      },
    ];
    expect(collectArtifactsFromSteps(steps, ["D:/projects/AgentZ"])).toEqual([
      "MODULE_DEPENDENCY_GRAPH.md",
    ]);
  });
});

describe("filterArtifactPaths (progressive filter)", () => {
  const paths = ["docs/plan.md", "src/app.tsx", "docs/api-plan.md", "README.md"];

  it("narrows as more tokens are typed", () => {
    expect(filterArtifactPaths(paths, "plan")).toEqual(["docs/plan.md", "docs/api-plan.md"]);
    expect(filterArtifactPaths(paths, "docs plan")).toEqual([
      "docs/plan.md",
      "docs/api-plan.md",
    ]);
    expect(filterArtifactPaths(paths, "docs api")).toEqual(["docs/api-plan.md"]);
  });

  it("is case-insensitive and matches the empty query as everything", () => {
    expect(filterArtifactPaths(paths, "README")).toEqual(["README.md"]);
    expect(filterArtifactPaths(paths, "  ")).toEqual(paths);
  });
});

describe("sortArtifactPaths", () => {
  const newestFirst = ["c/third.md", "a/first.md", "b/second.md"];

  it("time-desc keeps the incoming newest-first order", () => {
    expect(sortArtifactPaths(newestFirst, "time-desc")).toEqual(newestFirst);
    expect(sortArtifactPaths(newestFirst, "time-asc")).toEqual([
      "b/second.md",
      "a/first.md",
      "c/third.md",
    ]);
  });

  it("sorts by basename and by path", () => {
    expect(sortArtifactPaths(newestFirst, "name-asc")).toEqual([
      "a/first.md",
      "b/second.md",
      "c/third.md",
    ]);
    expect(sortArtifactPaths(newestFirst, "name-desc")).toEqual([
      "c/third.md",
      "b/second.md",
      "a/first.md",
    ]);
    expect(sortArtifactPaths(newestFirst, "path-asc")).toEqual([
      "a/first.md",
      "b/second.md",
      "c/third.md",
    ]);
  });
});
