// @vitest-environment jsdom
import { render, cleanup, fireEvent, act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ForkMenu } from "./AssistantMessageList";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("./Markdown", () => ({ default: () => null }));
vi.mock("../../components/ToolTrace", () => ({ default: () => null, interleaveTools: () => [] }));

afterEach(() => cleanup());

function rect(top: number, height = 20): DOMRect {
  return {
    top,
    bottom: top + height,
    left: 40,
    right: 140,
    width: 100,
    height,
    x: 40,
    y: top,
    toJSON: () => ({}),
  } as DOMRect;
}

/** Mount the menu inside a stand-in for the scrolling message list. */
function mount(triggerTop: number, onFork = vi.fn()) {
  Object.defineProperty(window, "innerHeight", { value: 600, configurable: true });
  Object.defineProperty(window, "innerWidth", { value: 800, configurable: true });
  const view = render(
    <div className="agentz-assistant-messages" style={{ overflowY: "auto" }}>
      <ForkMenu onFork={onFork} />
    </div>,
  );
  const trigger = view.container.querySelector<HTMLButtonElement>(".agentz-checkpoint-btn")!;
  trigger.getBoundingClientRect = () => rect(triggerTop);
  return { view, trigger, onFork };
}

const popover = () => document.querySelector<HTMLElement>(".agentz-fork-popover");

describe("ForkMenu popover", () => {
  it("renders outside the scrolling message list so the list cannot clip it", () => {
    const { view, trigger } = mount(300);
    act(() => {
      fireEvent.click(trigger);
    });
    const node = popover();
    expect(node, "popover must open").toBeTruthy();
    const list = view.container.querySelector(".agentz-assistant-messages")!;
    expect(list.contains(node), "popover must not be a descendant of the scroll container").toBe(false);
    expect(node!.parentElement).toBe(document.body);
    expect(node!.style.position).toBe("fixed");
  });

  it("opens upward when there is room above the button", () => {
    const { trigger } = mount(300);
    act(() => {
      fireEvent.click(trigger);
    });
    const node = popover()!;
    // Anchored from the bottom: its lower edge sits just above the button.
    expect(node.style.top).toBe("auto");
    expect(node.style.bottom).toBe("306px");
  });

  it("flips below the button when the button sits at the top of the window", () => {
    const { trigger } = mount(4);
    act(() => {
      fireEvent.click(trigger);
    });
    const node = popover()!;
    expect(node.style.bottom).toBe("auto");
    expect(node.style.top).toBe("30px");
  });

  it("still reports the choice and closes when an item is clicked", () => {
    const { trigger, onFork } = mount(300);
    act(() => {
      fireEvent.click(trigger);
    });
    const items = popover()!.querySelectorAll("button");
    act(() => {
      // A click inside the portal must not count as an outside click.
      fireEvent.mouseDown(items[1]);
      fireEvent.click(items[1]);
    });
    expect(onFork).toHaveBeenCalledWith(true);
    expect(popover()).toBeNull();
  });

  it("closes on outside click, Escape, and scroll of the conversation", () => {
    const { view, trigger } = mount(300);
    const open = () =>
      act(() => {
        fireEvent.click(trigger);
      });

    open();
    act(() => {
      fireEvent.mouseDown(document.body);
    });
    expect(popover()).toBeNull();

    open();
    act(() => {
      fireEvent.keyDown(document, { key: "Escape" });
    });
    expect(popover()).toBeNull();

    open();
    act(() => {
      fireEvent.scroll(view.container.querySelector(".agentz-assistant-messages")!);
    });
    expect(popover()).toBeNull();
  });
});
