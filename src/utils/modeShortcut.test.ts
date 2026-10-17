import { describe, expect, it } from "vitest";
import { modeShortcutLabel, modeShortcutTarget, type ModeKeyEvent } from "./modeShortcut";

function ev(partial: Partial<ModeKeyEvent>): ModeKeyEvent {
  return { key: "n", ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...partial };
}

describe("modeShortcutTarget", () => {
  it("Ctrl+Shift+N opens the IDE (codez)", () => {
    expect(modeShortcutTarget(ev({ ctrlKey: true, shiftKey: true }))).toBe("codez");
  });

  it("Ctrl+Alt+N opens the Agents window (workz)", () => {
    expect(modeShortcutTarget(ev({ ctrlKey: true, altKey: true }))).toBe("workz");
  });

  it("Cmd on macOS maps the same way", () => {
    expect(modeShortcutTarget(ev({ metaKey: true, shiftKey: true }))).toBe("codez");
    expect(modeShortcutTarget(ev({ metaKey: true, altKey: true }))).toBe("workz");
  });

  it("is case-insensitive for the letter key", () => {
    expect(modeShortcutTarget(ev({ key: "N", ctrlKey: true, shiftKey: true }))).toBe("codez");
  });

  it("ignores plain N, other letters, and missing modifier", () => {
    expect(modeShortcutTarget(ev({}))).toBeNull();
    expect(modeShortcutTarget(ev({ ctrlKey: true }))).toBeNull();
    expect(modeShortcutTarget(ev({ key: "m", ctrlKey: true, shiftKey: true }))).toBeNull();
  });

  it("ignores ambiguous combinations with both Shift and Alt", () => {
    expect(modeShortcutTarget(ev({ ctrlKey: true, shiftKey: true, altKey: true }))).toBeNull();
  });
});

describe("modeShortcutLabel", () => {
  it("shows platform-specific labels", () => {
    expect(modeShortcutLabel("codez", false)).toBe("Ctrl+Shift+N");
    expect(modeShortcutLabel("workz", false)).toBe("Ctrl+Alt+N");
    expect(modeShortcutLabel("codez", true)).toBe("⌘⇧N");
    expect(modeShortcutLabel("workz", true)).toBe("⌘⌥N");
  });
});
