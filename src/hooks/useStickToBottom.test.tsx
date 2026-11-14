// @vitest-environment jsdom
import { createRef } from "react";
import { act, create } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useStickToBottom } from "./useStickToBottom";

/** Minimal scrollable-container stub: jsdom has no layout, so scrollHeight /
 *  clientHeight are modelled explicitly. */
function makeEl(scrollHeight: number, clientHeight = 100) {
  const listeners = new Map<string, Set<(e?: unknown) => void>>();
  const el = {
    scrollTop: 0,
    scrollHeight,
    clientHeight,
    addEventListener(type: string, fn: (e?: unknown) => void) {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type)!.add(fn);
    },
    removeEventListener(type: string, fn: (e?: unknown) => void) {
      listeners.get(type)?.delete(fn);
    },
    emit(type: string) {
      listeners.get(type)?.forEach((fn) => fn());
    },
    growBy(px: number) {
      el.scrollHeight += px;
    },
  };
  return el;
}

type El = ReturnType<typeof makeEl>;

let observers: FakeMO[] = [];
class FakeMO {
  cb: () => void;
  disconnected = false;
  constructor(cb: () => void) {
    this.cb = cb;
    observers.push(this);
  }
  observe() {}
  disconnect() {
    this.disconnected = true;
  }
  fire() {
    if (!this.disconnected) this.cb();
  }
}
vi.stubGlobal("MutationObserver", FakeMO);

let rafQueue: FrameRequestCallback[] = [];
vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
  rafQueue.push(cb);
  return rafQueue.length;
});
vi.stubGlobal("cancelAnimationFrame", () => {});
function flushRaf() {
  const q = rafQueue;
  rafQueue = [];
  q.forEach((cb) => cb(0));
}

function Harness({
  el,
  resetKey,
  apiRef,
}: {
  el: El | null;
  resetKey?: string;
  apiRef: { current: ReturnType<typeof useStickToBottom> | null };
}) {
  const ref = createRef<HTMLElement>();
  (ref as { current: HTMLElement | null }).current = el as unknown as HTMLElement | null;
  apiRef.current = useStickToBottom(ref, resetKey);
  return null;
}

afterEach(() => {
  observers = [];
  rafQueue = [];
});

describe("useStickToBottom session reset", () => {
  it("pins to the newest message when the reset key changes, even if the user scrolled up before", async () => {
    const el = makeEl(1000);
    const apiRef = { current: null as ReturnType<typeof useStickToBottom> | null };
    let renderer!: ReturnType<typeof create>;
    await act(async () => {
      renderer = create(<Harness el={el} resetKey="session-A" apiRef={apiRef} />);
    });
    flushRaf();
    expect(el.scrollTop).toBe(1000);

    // User scrolls up in session A → follow disabled.
    await act(async () => {
      el.scrollTop = 100;
      el.emit("scroll");
    });
    expect(apiRef.current!.stickRef.current).toBe(false);

    // Switch to session B: same container, new content height, new reset key.
    el.scrollHeight = 2000;
    el.scrollTop = 100; // stale position from session A
    await act(async () => {
      renderer.update(<Harness el={el} resetKey="session-B" apiRef={apiRef} />);
    });
    flushRaf();

    expect(apiRef.current!.stickRef.current, "follow must re-arm on session switch").toBe(true);
    expect(el.scrollTop, "must land on the newest message, not the stale top").toBe(2000);
    await act(async () => renderer.unmount());
  });

  it("keeps following growth after a session switch", async () => {
    const el = makeEl(500);
    const apiRef = { current: null as ReturnType<typeof useStickToBottom> | null };
    let renderer!: ReturnType<typeof create>;
    await act(async () => {
      renderer = create(<Harness el={el} resetKey="a" apiRef={apiRef} />);
    });
    flushRaf();
    await act(async () => {
      el.scrollTop = 0;
      el.emit("scroll");
    });
    await act(async () => {
      renderer.update(<Harness el={el} resetKey="b" apiRef={apiRef} />);
    });
    flushRaf();

    // New content streams in → mutation observer must keep the bottom visible.
    await act(async () => {
      el.growBy(300);
      observers.filter((o) => !o.disconnected).forEach((o) => o.fire());
    });
    flushRaf();
    expect(el.scrollTop).toBe(800);
    await act(async () => renderer.unmount());
  });

  it("does not yank the user back down mid-session when they scrolled up", async () => {
    const el = makeEl(1000);
    const apiRef = { current: null as ReturnType<typeof useStickToBottom> | null };
    let renderer!: ReturnType<typeof create>;
    await act(async () => {
      renderer = create(<Harness el={el} resetKey="a" apiRef={apiRef} />);
    });
    flushRaf();
    await act(async () => {
      el.scrollTop = 200;
      el.emit("scroll");
    });
    // Same session, content grows: no forced pin.
    await act(async () => {
      el.growBy(100);
      observers.filter((o) => !o.disconnected).forEach((o) => o.fire());
    });
    flushRaf();
    expect(el.scrollTop).toBe(200);
    await act(async () => renderer.unmount());
  });
});
