import { describe, expect, it, vi, afterEach } from "vitest";
import { satisfies } from "../../extensions/semver";

vi.mock("./vsix", async (importOriginal) => {
  // Re-import real module symbols we don't stub — only mock fetch via global.
  return await importOriginal<typeof import("./vsix")>();
});

import { openVsxPickCompatible } from "./vsix";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("openVsxPickCompatible", () => {
  it("prefers a older compatible build when latest engines are too new", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        const u = String(url);
        if (u.endsWith("/versions")) {
          return {
            ok: true,
            json: async () => ({ versions: { "2.0.0": "", "1.5.0": "", "1.0.0": "" } }),
          };
        }
        if (u.endsWith("/2.0.0")) {
          return {
            ok: true,
            json: async () => ({
              files: { download: "https://example/2.0.0.vsix" },
              engines: { vscode: "^1.200.0" },
            }),
          };
        }
        if (u.endsWith("/1.5.0")) {
          return {
            ok: true,
            json: async () => ({
              files: { download: "https://example/1.5.0.vsix" },
              engines: { vscode: "^1.90.0" },
            }),
          };
        }
        return { ok: false, status: 404, json: async () => ({}) };
      }),
    );

    const pick = await openVsxPickCompatible("pub", "ext", "1.105.0", "2.0.0", satisfies);
    expect(pick.version).toBe("1.5.0");
    expect(pick.compatible).toBe(true);
    expect(pick.fellBack).toBe(true);
    expect(pick.download).toContain("1.5.0");
  });

  it("returns latest with compatible=false when no version matches", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        const u = String(url);
        if (u.endsWith("/versions")) {
          return { ok: true, json: async () => ({ versions: { "9.0.0": "" } }) };
        }
        return {
          ok: true,
          json: async () => ({
            files: { download: "https://example/9.0.0.vsix" },
            engines: { vscode: "^1.200.0" },
          }),
        };
      }),
    );

    const pick = await openVsxPickCompatible("pub", "ext", "1.105.0", undefined, satisfies);
    expect(pick.version).toBe("9.0.0");
    expect(pick.compatible).toBe(false);
    expect(pick.fellBack).toBe(false);
  });
});
