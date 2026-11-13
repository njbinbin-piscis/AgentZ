// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn().mockResolvedValue(undefined);
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("../i18n", () => ({
  default: { t: (key: string) => key },
}));

import {
  buildDefaultContextMenuItems,
  installNativeContextMenuGuard,
  isEditableTarget,
} from "./contextMenuGuard";

function fireContextMenu(target: Element, opts: { prevent?: boolean } = {}): MouseEvent {
  const e = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 10, clientY: 20 });
  if (opts.prevent) e.preventDefault();
  target.dispatchEvent(e);
  return e;
}

beforeEach(() => {
  document.body.innerHTML = "";
  invoke.mockClear();
});
afterEach(() => {
  document.body.innerHTML = "";
});

describe("installNativeContextMenuGuard", () => {
  it("suppresses the browser menu and opens the generic menu on plain surfaces", () => {
    const open = vi.fn();
    const uninstall = installNativeContextMenuGuard(open);
    const div = document.createElement("div");
    document.body.appendChild(div);
    const e = fireContextMenu(div);
    expect(e.defaultPrevented).toBe(true);
    expect(open).toHaveBeenCalledTimes(1);
    const [x, y, items] = open.mock.calls[0];
    expect(x).toBe(10);
    expect(y).toBe(20);
    expect(items.some((i: { id: string }) => i.id === "copy")).toBe(true);
    uninstall();
  });

  it("stands down when a component already handled the event", () => {
    const open = vi.fn();
    const uninstall = installNativeContextMenuGuard(open);
    const div = document.createElement("div");
    document.body.appendChild(div);
    // A React-style component handler runs at the root before document bubble.
    const rootHandler = (e: Event) => e.preventDefault();
    document.body.addEventListener("contextmenu", rootHandler);
    fireContextMenu(div);
    expect(open).not.toHaveBeenCalled();
    document.body.removeEventListener("contextmenu", rootHandler);
    uninstall();
  });

  it("keeps Monaco's native editor menu", () => {
    const open = vi.fn();
    const uninstall = installNativeContextMenuGuard(open);
    const editor = document.createElement("div");
    editor.className = "monaco-editor";
    document.body.appendChild(editor);
    const e = fireContextMenu(editor);
    expect(e.defaultPrevented).toBe(false);
    expect(open).not.toHaveBeenCalled();
    uninstall();
  });

  it("honors data-native-context-menu opt-out", () => {
    const open = vi.fn();
    const uninstall = installNativeContextMenuGuard(open);
    const el = document.createElement("div");
    el.setAttribute("data-native-context-menu", "");
    document.body.appendChild(el);
    const e = fireContextMenu(el);
    expect(e.defaultPrevented).toBe(false);
    expect(open).not.toHaveBeenCalled();
    uninstall();
  });

  it("uninstall removes the guard", () => {
    const open = vi.fn();
    const uninstall = installNativeContextMenuGuard(open);
    uninstall();
    const div = document.createElement("div");
    document.body.appendChild(div);
    const e = fireContextMenu(div);
    expect(e.defaultPrevented).toBe(false);
    expect(open).not.toHaveBeenCalled();
  });
});

describe("buildDefaultContextMenuItems", () => {
  it("offers link actions for anchors", () => {
    const a = document.createElement("a");
    a.href = "https://example.com/page";
    document.body.appendChild(a);
    const items = buildDefaultContextMenuItems({ target: a } as unknown as MouseEvent);
    expect(items.some((i) => i.id === "open-link")).toBe(true);
    expect(items.some((i) => i.id === "copy-link")).toBe(true);
  });

  it("offers copy/paste/cut/select-all for editable inputs", () => {
    const input = document.createElement("input");
    input.type = "text";
    input.value = "hello world";
    document.body.appendChild(input);
    const items = buildDefaultContextMenuItems({ target: input } as unknown as MouseEvent);
    const ids = items.map((i) => i.id);
    expect(ids).toContain("copy");
    expect(ids).toContain("paste");
    expect(ids).toContain("cut");
    expect(ids).toContain("select-all");
  });

  it("disables copy when there is no selection", () => {
    const div = document.createElement("div");
    document.body.appendChild(div);
    const items = buildDefaultContextMenuItems({ target: div } as unknown as MouseEvent);
    const copy = items.find((i) => i.id === "copy");
    expect(copy?.disabled).toBe(true);
  });

  it("offers copy-image for data-URI images instead of a giant address", () => {
    const img = document.createElement("img");
    img.src = "data:image/png;base64,AAAA";
    document.body.appendChild(img);
    const items = buildDefaultContextMenuItems({ target: img } as unknown as MouseEvent);
    expect(items.some((i) => i.id === "copy-image")).toBe(true);
    expect(items.some((i) => i.id === "copy-image-src")).toBe(false);
  });

  it("offers copy-image-address for remote images", () => {
    const img = document.createElement("img");
    img.src = "https://example.com/pic.png";
    document.body.appendChild(img);
    const items = buildDefaultContextMenuItems({ target: img } as unknown as MouseEvent);
    expect(items.some((i) => i.id === "copy-image-src")).toBe(true);
  });
});

describe("isEditableTarget", () => {
  it("recognizes text inputs and textareas, rejects readonly/disabled", () => {
    const input = document.createElement("input");
    expect(isEditableTarget(input)).toBe(true);
    input.readOnly = true;
    expect(isEditableTarget(input)).toBe(false);
    const ta = document.createElement("textarea");
    expect(isEditableTarget(ta)).toBe(true);
    const div = document.createElement("div");
    expect(isEditableTarget(div)).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});
