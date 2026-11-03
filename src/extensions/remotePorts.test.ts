import { expect, it } from "vitest";
import { remoteLocalhostPort, urlOnForward } from "./remoteBrowserUrl";

it("reads the port from a loopback preview url", () => {
  expect(remoteLocalhostPort("http://localhost:3000/app")).toBe(3000);
  expect(remoteLocalhostPort("http://127.0.0.1:5173")).toBe(5173);
  expect(remoteLocalhostPort("https://example.com")).toBeNull();
});

it("rewrites a loopback url onto the forwarded local port", () => {
  expect(urlOnForward("http://localhost:3000/app?x=1", 41234)).toBe("http://127.0.0.1:41234/app?x=1");
});
