import { expect, it } from "vitest";
import { tabNavigationTarget } from "./tabNavigation";
it("keyboard navigation wraps visible tabs and reaches both ends", () => {
  const tabs = ["terminal", "output", "debug"];
  expect(tabNavigationTarget(tabs, "terminal", "ArrowLeft")).toBe("debug");
  expect(tabNavigationTarget(tabs, "debug", "ArrowRight")).toBe("terminal");
  expect(tabNavigationTarget(tabs, "output", "Home")).toBe("terminal");
  expect(tabNavigationTarget(tabs, "output", "End")).toBe("debug");
});
it("empty/removed tab and ordinary keys are handled without inventing a target", () => {
  expect(tabNavigationTarget([], "terminal", "Home")).toBeNull();
  expect(tabNavigationTarget(["terminal"], "removed", "ArrowLeft")).toBe("terminal");
  expect(tabNavigationTarget(["terminal"], "terminal", "Tab")).toBeNull();
});
