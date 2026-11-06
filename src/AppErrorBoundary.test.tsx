import { act, create, type ReactTestRenderer } from "react-test-renderer";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import AppErrorBoundary from "./AppErrorBoundary";

function Boom(): ReactElement {
  throw new Error("preview exploded");
}

describe("AppErrorBoundary", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("keeps a recovery screen when a child render throws", async () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    await act(async () => {
      root = create(
        <AppErrorBoundary>
          <Boom />
        </AppErrorBoundary>,
      );
    });
    const text = root!.root.findAllByType("h1").map((n) => String(n.children.join(""))).join("");
    expect(text.length).toBeGreaterThan(0);
    expect(root!.root.findAllByType("button").length).toBe(2);
    spy.mockRestore();
  });
});
