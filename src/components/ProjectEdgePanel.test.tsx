import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProjectEdgeProvider, useProjectEdgeActions } from "../contexts/ProjectEdgeContext";
import ProjectEdgePanel from "./ProjectEdgePanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && "filtered" in opts ? `${opts.filtered}/${opts.total}` : key,
  }),
}));

let actionsRef: ReturnType<typeof useProjectEdgeActions> | null = null;

function CaptureActions() {
  actionsRef = useProjectEdgeActions();
  return null;
}

function artifactNames(root: ReactTestRenderer): string[] {
  return root.root
    .findAllByProps({ className: "agentz-edge-artifact-name" })
    .map((n) => String(n.children[0]));
}

describe("ProjectEdgePanel artifacts", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => root?.unmount());
      root = null;
    }
    actionsRef = null;
  });

  async function renderWithArtifacts(paths: string[]) {
    await act(async () => {
      root = create(
        <ProjectEdgeProvider>
          <CaptureActions />
          <ProjectEdgePanel projectDir="/repo" />
        </ProjectEdgeProvider>,
      );
    });
    await act(async () => {
      actionsRef?.setArtifacts(paths);
    });
    // No git changes → the panel lands on the artifacts tab.
    await act(async () => {});
    return root!;
  }

  it("lists artifacts newest-first by default (time-desc)", async () => {
    const r = await renderWithArtifacts(["c/third.md", "a/first.md", "b/second.md"]);
    expect(artifactNames(r)).toEqual(["third.md", "first.md", "second.md"]);

    const select = r.root.findByProps({ "data-testid": "artifact-sort-select" });
    expect(select.props.value).toBe("time-desc");
  });

  it("progressively filters by tokens and can re-sort by name", async () => {
    const r = await renderWithArtifacts([
      "docs/plan.md",
      "src/app.tsx",
      "docs/api-plan.md",
    ]);

    const input = r.root.findByProps({ "data-testid": "artifact-filter-input" });
    await act(async () => input.props.onChange({ target: { value: "plan" } }));
    expect(artifactNames(r)).toEqual(["plan.md", "api-plan.md"]);
    expect(
      r.root.findByProps({ "data-testid": "artifact-filter-count" }).children.join(""),
    ).toBe("2/3");

    await act(async () =>
      r.root
        .findByProps({ "data-testid": "artifact-filter-input" })
        .props.onChange({ target: { value: "docs api" } }),
    );
    expect(artifactNames(r)).toEqual(["api-plan.md"]);

    // Clearing the filter restores the full list; name sort then applies.
    await act(async () =>
      r.root.findByProps({ "data-testid": "artifact-filter-clear" }).props.onClick(),
    );
    expect(artifactNames(r)).toEqual(["plan.md", "app.tsx", "api-plan.md"]);

    await act(async () =>
      r.root
        .findByProps({ "data-testid": "artifact-sort-select" })
        .props.onChange({ target: { value: "name-asc" } }),
    );
    expect(artifactNames(r)).toEqual(["api-plan.md", "app.tsx", "plan.md"]);

    await act(async () =>
      r.root
        .findByProps({ "data-testid": "artifact-sort-select" })
        .props.onChange({ target: { value: "time-asc" } }),
    );
    // Oldest first = exact reverse of the newest-first arrival order.
    expect(artifactNames(r)).toEqual(["api-plan.md", "app.tsx", "plan.md"]);
  });

  it("shows a no-match empty state when the filter excludes everything", async () => {
    const r = await renderWithArtifacts(["docs/plan.md"]);
    await act(async () =>
      r.root
        .findByProps({ "data-testid": "artifact-filter-input" })
        .props.onChange({ target: { value: "zzz-no-match" } }),
    );
    expect(artifactNames(r)).toEqual([]);
    expect(JSON.stringify(r.toJSON())).toContain("agent.noMatchingArtifacts");
  });

  it("opens the exact artifact path that was listed when clicked", async () => {
    const r = await renderWithArtifacts(["MODULE_DEPENDENCY_GRAPH.md", "docs/plan.md"]);
    const items = r.root.findAllByProps({ className: "agentz-edge-artifact-item" });
    expect(items).toHaveLength(2);
    expect(items[0].props.title).toBe("MODULE_DEPENDENCY_GRAPH.md");
    await act(async () => items[0].props.onClick());
    const after = r.root.findAllByProps({
      className: "agentz-edge-artifact-item active",
    });
    expect(after).toHaveLength(1);
    expect(after[0].props.title).toBe("MODULE_DEPENDENCY_GRAPH.md");
  });

  it("changes tab lists only pending agent edits, with per-file keep/undo", async () => {
    const onUndo = vi.fn(async () => {});
    const onKeep = vi.fn(async () => {});
    await act(async () => {
      root = create(
        <ProjectEdgeProvider>
          <CaptureActions />
          <ProjectEdgePanel projectDir="/repo" onUndo={onUndo} onKeep={onKeep} />
        </ProjectEdgeProvider>,
      );
    });
    await act(async () => {
      actionsRef?.setGitChanges([{ path: "untracked.txt", status: "untracked", staged: false }]);
      actionsRef?.setPendingReview({
        sessionId: "s",
        changes: [
          { id: 2, rel_path: "src/a.ts", tool_name: "file_edit", existed: true, applied: true, status: "modified", turn_id: "t" },
          { id: 1, rel_path: "old.md", tool_name: "file_write", existed: true, applied: true, status: "deleted", turn_id: "t" },
        ],
      });
    });
    const json = JSON.stringify(root!.toJSON());
    expect(json).not.toContain("untracked.txt");
    const rows = root!.root.findAllByProps({ className: "agentz-edge-change" });
    expect(rows.map((r) => r.props.title)).toEqual(["src/a.ts", "old.md (agent.changeDeleted)"]);
    expect(rows[1].props.disabled).toBe(true);
    const acts = root!.root.findAllByProps({ className: "agentz-edge-change-act" });
    await act(async () => acts[0].props.onClick());
    await act(async () => acts[1].props.onClick());
    expect(onUndo).toHaveBeenCalledWith(["src/a.ts"]);
    expect(onKeep).toHaveBeenCalledWith(["src/a.ts"]);
  });
});
