import { describe, expect, it } from "vitest";
import { absPathToProjectRel } from "./chatComposerUtils";
import { createFileRefChip, fileRefPlaceholder } from "./composerChips";

describe("absPathToProjectRel", () => {
  it("maps paths inside the project to relative paths", () => {
    expect(absPathToProjectRel("C:\\proj\\src\\a.ts", "C:\\proj")).toBe("src/a.ts");
    expect(absPathToProjectRel("c:/PROJ/src/a.ts", "C:\\proj\\")).toBe("src/a.ts");
    expect(absPathToProjectRel("/home/me/proj/a.ts", "/home/me/proj")).toBe("a.ts");
  });

  it("keeps the full path for files outside the project instead of the bare name", () => {
    expect(absPathToProjectRel("D:\\docs\\spec.pdf", "C:\\proj")).toBe("D:\\docs\\spec.pdf");
    expect(absPathToProjectRel("/tmp/x/y.txt", "/home/me/proj")).toBe("/tmp/x/y.txt");
  });

  it("does not treat a sibling directory sharing the prefix as inside", () => {
    expect(absPathToProjectRel("/home/me/proj2/a.ts", "/home/me/proj")).toBe("/home/me/proj2/a.ts");
  });

  it("keeps the leading slash of an external absolute path in the placeholder", () => {
    const chip = createFileRefChip("/tmp/x/y.txt", false, true);
    expect(fileRefPlaceholder(chip.path, chip.absolute)).toBe("@/tmp/x/y.txt");
    expect(fileRefPlaceholder("/src/a.ts")).toBe("@src/a.ts");
  });
});
