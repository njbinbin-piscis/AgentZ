import { describe, expect, it } from "vitest";
import { satisfies } from "./semver";
import { HOST_VSCODE_VERSION } from "./common/protocol";

describe("HOST_VSCODE_VERSION soft-compat", () => {
  it("is at least 1.105 so common ^1.9x / ^1.10x engines pass", () => {
    expect(satisfies(HOST_VSCODE_VERSION, "^1.96.0")).toBe(true);
    expect(satisfies(HOST_VSCODE_VERSION, "^1.100.0")).toBe(true);
    expect(satisfies(HOST_VSCODE_VERSION, "^1.105.0")).toBe(true);
  });

  it("still reports engines that require a future host as unsatisfied", () => {
    expect(satisfies(HOST_VSCODE_VERSION, "^1.200.0")).toBe(false);
  });
});
