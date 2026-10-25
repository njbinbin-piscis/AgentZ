import { describe, expect, it } from "vitest";
import { redirectTargets } from "./bgRedirect";

const cwd = "C:/proj";

describe("redirectTargets", () => {
  it("finds stdout redirects and skips stream merges", () => {
    expect(redirectTargets("python -m pytest -q > pytest_cov.log 2>&1", cwd)).toEqual(["C:/proj/pytest_cov.log"]);
  });

  it("handles append, stderr, quotes and absolute paths", () => {
    expect(redirectTargets('a >> out.txt 2> "logs/err.txt"', cwd)).toEqual(["C:/proj/out.txt", "C:/proj/logs/err.txt"]);
    expect(redirectTargets("a > D:\\tmp\\x.log", cwd)).toEqual(["D:\\tmp\\x.log"]);
  });

  it("ignores null sinks and plain commands", () => {
    expect(redirectTargets("chcp 65001 >nul & a > $null", cwd)).toEqual([]);
    expect(redirectTargets("python -m pytest -q", cwd)).toEqual([]);
  });

  it("detects PowerShell file sinks", () => {
    expect(redirectTargets("npm test | Tee-Object -FilePath test.log", cwd)).toEqual(["C:/proj/test.log"]);
    expect(redirectTargets("npm test | Out-File .\\r.txt", cwd)).toEqual(["C:/proj/r.txt"]);
  });
});
