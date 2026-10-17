import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, expect, it, vi } from "vitest";

const { onSelectPath } = vi.hoisted(() => ({ onSelectPath: vi.fn() }));

vi.mock("../debug/debugController", () => ({
  debugController: {
    projectRelativePath: (source?: string) =>
      source?.startsWith("/project/") ? source.slice("/project/".length) : null,
    continue: vi.fn(),
    stepOver: vi.fn(),
    stepInto: vi.fn(),
    stepOut: vi.fn(),
    stop: vi.fn(),
  },
}));
vi.mock("../../contexts/ProjectEdgeContext", () => ({
  useProjectEdgeActions: () => ({ onSelectPath }),
}));
vi.mock("./useExtensionUi", () => ({ useExtensionUi: () => ({ debugOutput: [] }) }));

import DebugView from "./DebugView";
import { debugStore } from "../debug/debugStore";

afterEach(() => {
  debugStore.end();
  onSelectPath.mockClear();
});

it("opens in-project stack frames and labels frames outside the project", () => {
  debugStore.start();
  debugStore.setStopped(1, [
    { id: 1, name: "inside", line: 3, source: "/project/src/a.ts" },
    { id: 2, name: "outside", line: 4, source: "/elsewhere/b.ts" },
  ]);
  let renderer: ReactTestRenderer | undefined;
  act(() => {
    renderer = create(<DebugView />);
  });
  const openers = renderer!.root.findAll(
    (node) => node.type === "button" && node.props.className === "agentz-debug-frame-open",
  );
  expect(openers).toHaveLength(1);
  act(() => openers[0].props.onClick());
  expect(onSelectPath).toHaveBeenCalledWith("src/a.ts");
  expect(JSON.stringify(renderer!.toJSON())).toContain("outside project");
  act(() => renderer!.unmount());
});
