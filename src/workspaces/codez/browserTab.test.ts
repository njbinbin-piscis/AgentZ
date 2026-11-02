import { describe, expect, it } from "vitest";
import { localFileUrl } from "./browserTab";

describe("localFileUrl", () => {
  it("builds file URLs for Windows, POSIX and UNC paths", () => {
    expect(localFileUrl("C:\\my proj\\a#b.html")).toBe("file:///C:/my%20proj/a%23b.html");
    expect(localFileUrl("/home/u/x y.html")).toBe("file:///home/u/x%20y.html");
    expect(localFileUrl("\\\\host\\share\\p.html")).toBe("file://host/share/p.html");
  });
});
