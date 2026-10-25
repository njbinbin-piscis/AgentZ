import { describe, expect, it } from "vitest";
import { capabilitiesFor } from "./remoteTargets";

describe("capabilitiesFor", () => {
  it("allows everything for local projects", () => {
    expect(capabilitiesFor("C:/work/app")).toEqual({ ide: true, chat: true, teams: true, rooms: true });
  });

  it("keeps team pools and rooms local-only", () => {
    expect(capabilitiesFor("agentz-remote://ssh-remote+box/srv/app")).toEqual({
      ide: true,
      chat: true,
      teams: false,
      rooms: false,
    });
  });
});
