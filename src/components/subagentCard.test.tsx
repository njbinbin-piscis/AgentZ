import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";
import ToolTrace, { type ToolTraceItem } from "./ToolTrace";
import {
  applySubagentProgress,
  isSubagentCall,
  parseSubagent,
  settleSubagentActivity,
  type SubagentProgress,
} from "./subagentCard";

vi.mock("react-i18next", () => {
  const hook = {
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
  };
  return { useTranslation: () => hook };
});

const workspaceSources = import.meta.glob(
  ["../workspaces/codez/AssistantMessageList.tsx", "../workspaces/workz/WorkzMessageStream.tsx"],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;

function item(over: Partial<ToolTraceItem>): ToolTraceItem {
  return { id: "1", name: "task", status: "running", ...over };
}

describe("parseSubagent", () => {
  it("reads agent, brief and stats from a task report", () => {
    const v = parseSubagent(
      item({
        status: "done",
        input: { action: "call", agent_id: "scout", brief: "find callers of load_config" },
        result:
          "Scout report (5 tool calls, 1200+300 tokens, main-model fallback):\n\nsrc/a.rs:10 caller",
      }),
    );
    expect(v).toMatchObject({
      agent: "scout",
      brief: "find callers of load_config",
      toolCalls: 5,
      tokens: 1500,
      cached: false,
      fallback: true,
      report: "src/a.rs:10 caller",
    });
  });

  it("detects cached results", () => {
    const v = parseSubagent(
      item({
        status: "done",
        input: { agent_id: "scout", brief: "x" },
        result: "Scout report (cached, workspace unchanged):\n\nbody",
      }),
    );
    expect(v.cached).toBe(true);
    expect(v.report).toBe("body");
  });

  it("handles delegate and call_fish result shapes", () => {
    expect(
      parseSubagent(
        item({ name: "delegate", status: "done", input: { task: "t" }, result: "Sub-agent findings:\n\nR" }),
      ),
    ).toMatchObject({ agent: "research", brief: "t", report: "R" });
    expect(
      parseSubagent(
        item({ name: "call_fish", status: "done", input: { fish: "extractor", task: "t" }, result: "Extractor report:\n\nR2" }),
      ),
    ).toMatchObject({ agent: "extractor", report: "R2" });
  });

  it("tolerates streamed string input", () => {
    const v = parseSubagent(item({ input: '{"agent_id":"scout","brief":"hi' }));
    expect(v.agent).toBe("");
  });

  it("does not treat task action=list as a sub-agent run", () => {
    expect(isSubagentCall({ name: "task", input: { action: "list" } })).toBe(false);
    expect(isSubagentCall({ name: "task", input: { action: "call" } })).toBe(true);
    expect(isSubagentCall({ name: "file_read", input: {} })).toBe(false);
  });
});

function progress(over: Partial<SubagentProgress>): SubagentProgress {
  return {
    type: "subagent_progress",
    tool_id: "1",
    step_id: "think-1",
    iteration: 1,
    phase: "thinking",
    status: "running",
    name: "",
    detail: "",
    ...over,
  };
}

describe("sub-agent activity", () => {
  it("appends steps and updates the same step in place", () => {
    const started = applySubagentProgress(
      [item({ id: "1", name: "delegate", status: "running" })],
      progress({}),
    );
    const withTool = applySubagentProgress(started, progress({
      step_id: "call-a",
      phase: "tool",
      name: "file_read",
      detail: "desktop/src/execution/engine.rs",
    }));
    const finished = applySubagentProgress(withTool, progress({
      step_id: "call-a",
      phase: "tool",
      status: "done",
      name: "file_read",
      detail: "",
    }));
    expect(finished[0].activity).toMatchObject([
      { id: "think-1", status: "running" },
      { id: "call-a", status: "done", detail: "desktop/src/execution/engine.rs" },
    ]);
  });

  it("stops the spinner when the parent call finishes", () => {
    const settled = settleSubagentActivity(
      [
        { id: "think-1", iteration: 1, phase: "thinking", status: "done", name: "", detail: "" },
        { id: "call-a", iteration: 1, phase: "tool", status: "running", name: "file_read", detail: "a.rs" },
      ],
      true,
    );
    expect(settled?.map((step) => step.status)).toEqual(["done", "error"]);
  });
});

describe("SubagentCard rendering", () => {
  async function render(items: ToolTraceItem[]): Promise<ReactTestRenderer> {
    let root!: ReactTestRenderer;
    await act(async () => {
      root = create(<ToolTrace items={items} />);
    });
    return root;
  }
  const cards = (r: ReactTestRenderer) => r.root.findAllByProps({ "data-testid": "subagent-card" });

  it.each([
    ["running", "chat.subagentRunning"],
    ["done", "chat.subagentDone"],
    ["error", "chat.subagentFailed"],
  ] as const)("shows the %s state", async (status, label) => {
    const root = await render([
      item({ status, input: { agent_id: "scout", brief: "b" }, result: status === "running" ? undefined : "r" }),
    ]);
    expect(cards(root)).toHaveLength(1);
    expect(JSON.stringify(root.toJSON())).toContain(label);
  });

  it("shows the live step in the header and the full list when expanded", async () => {
    const [withActivity] = applySubagentProgress(
      [item({ id: "1", name: "delegate", status: "running", input: { task: "BRIEF-TEXT" } })],
      progress({ step_id: "call-a", phase: "tool", name: "file_read", detail: "engine.rs" }),
    );
    const root = await render([withActivity]);
    const collapsed = JSON.stringify(root.toJSON());
    expect(collapsed).toContain("file_read · engine.rs");
    expect(collapsed).not.toContain("chat.subagentActivity");
    const head = root.root.findByProps({ className: "agentz-subagent-head" });
    await act(async () => {
      head.props.onClick();
    });
    const open = JSON.stringify(root.toJSON());
    expect(open).toContain("chat.subagentActivity");
    expect(open).toContain("file_read · engine.rs");
  });

  it("expands to show brief and report", async () => {
    const root = await render([
      item({ status: "done", input: { agent_id: "scout", brief: "BRIEF-TEXT" }, result: "Scout report (1 tool calls, 1+1 tokens):\n\nREPORT-TEXT" }),
    ]);
    expect(JSON.stringify(root.toJSON())).not.toContain("REPORT-TEXT");
    const head = root.root.findByProps({ className: "agentz-subagent-head" });
    await act(async () => {
      head.props.onClick();
    });
    const out = JSON.stringify(root.toJSON());
    expect(out).toContain("BRIEF-TEXT");
    expect(out).toContain("REPORT-TEXT");
  });

  it("renders three parallel delegations as three independent cards", async () => {
    const root = await render([
      item({ id: "a", status: "done", input: { agent_id: "scout", brief: "one" }, result: "Scout report (1 tool calls, 1+1 tokens):\n\nA" }),
      item({ id: "b", status: "running", input: { agent_id: "extractor", brief: "two" } }),
      item({ id: "c", status: "error", name: "delegate", input: { task: "three" }, result: "boom" }),
    ]);
    const found = cards(root);
    expect(found).toHaveLength(3);
    const classes = found.map((c) => c.props.className as string);
    expect(classes[0]).toContain("is-done");
    expect(classes[1]).toContain("is-running");
    expect(classes[2]).toContain("is-error");
  });

  it("keeps ordinary tools in the normal batch", async () => {
    const root = await render([
      { id: "s", name: "shell", input: { command: "ls" }, status: "done" },
      item({ id: "t", status: "done", input: { agent_id: "scout", brief: "b" }, result: "r" }),
    ]);
    expect(cards(root)).toHaveLength(1);
    expect(JSON.stringify(root.toJSON())).toContain("agentz-tool-trace");
  });
});

describe("workspace wiring", () => {
  it("codez and workz both render tool calls through the shared ToolTrace", () => {
    const sources = Object.entries(workspaceSources);
    expect(sources).toHaveLength(2);
    for (const [file, src] of sources) {
      expect(src, file).toMatch(/ToolTrace/);
    }
  });
});
