import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { notifySettingsRefresh, onSettingsRefresh } from "./settingsRefresh";

describe("settingsRefresh", () => {
  const g = globalThis as { window?: unknown; CustomEvent?: unknown };
  const origWindow = g.window;
  const origCustomEvent = g.CustomEvent;

  beforeEach(() => {
    const target = new EventTarget();
    g.window = Object.assign(target, {
      dispatchEvent: target.dispatchEvent.bind(target),
    });
    g.CustomEvent ??= class extends Event {
      detail: unknown;
      constructor(type: string, init?: { detail?: unknown }) {
        super(type);
        this.detail = init?.detail;
      }
    };
  });
  afterEach(() => {
    if (origWindow === undefined) delete g.window;
    else g.window = origWindow;
    g.CustomEvent = origCustomEvent;
  });

  it("tells listeners whether the refresh was a real save or a silent reload", () => {
    const seen = vi.fn();
    const off = onSettingsRefresh(seen);
    notifySettingsRefresh();
    notifySettingsRefresh({ silent: true });
    off();
    notifySettingsRefresh();
    expect(seen.mock.calls).toEqual([[{ silent: false }], [{ silent: true }]]);
  });
});
