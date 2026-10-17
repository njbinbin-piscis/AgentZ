import { describe, expect, it } from "vitest";
import { saveBlockReason } from "./saveBlock";

describe("saveBlockReason", () => {
  it("allows saving an ordinary writable tab", () => {
    expect(saveBlockReason("src/app.ts", { isReadOnly: false, isDiff: false })).toBeNull();
    expect(saveBlockReason("src/app.ts", undefined)).toBeNull();
  });

  it("blocks diff tabs by path prefix or flag", () => {
    expect(saveBlockReason("diff:src/app.ts", undefined)).toBe("diff");
    expect(saveBlockReason("src/app.ts", { isDiff: true })).toBe("diff");
  });

  it("blocks read-only tabs", () => {
    expect(saveBlockReason("src/app.ts", { isReadOnly: true })).toBe("readOnly");
  });

  it("blocks recovery tabs even when they are not flagged read-only", () => {
    const recovery = "agentz-recovery:src/app.ts";
    expect(saveBlockReason(recovery, { isReadOnly: false })).toBe("recovery");
  });
});
