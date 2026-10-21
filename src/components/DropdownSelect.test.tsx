import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import DropdownSelect, {
  DropdownMultiSelect,
  computeContextMenuPosition,
  computeFixedPopupPosition,
} from "./DropdownSelect";
import {
  DEFAULT_EDITOR_FONT_SIZE,
  UI_FONT_SCALES,
  getEffectiveEditorFontSize,
  getEffectiveTerminalFontSize,
} from "../theme";

describe("DropdownSelect & DropdownMultiSelect", () => {
  let root: ReactTestRenderer | null = null;
  const origWindow = (globalThis as { window?: unknown }).window;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
    if (origWindow === undefined) {
      delete (globalThis as { window?: unknown }).window;
    } else {
      (globalThis as { window?: unknown }).window = origWindow;
    }
  });

  it("opens single-select listbox, renders options with hints, and selects option on click", async () => {
    const onChange = vi.fn();
    await act(async () => {
      root = create(
        <DropdownSelect
          variant="text"
          placement="up"
          value="gpt-5"
          onChange={onChange}
          options={[
            { id: "gpt-5", label: "GPT-5", hint: "Default reasoning model" },
            { id: "claude-4", label: "Claude 4", hint: "Coding model" },
          ]}
        />,
      );
    });

    const trigger = root!.root.findByProps({ "aria-haspopup": "listbox" });
    expect(trigger.props["aria-expanded"]).toBe(false);

    await act(async () => {
      trigger.props.onClick();
    });

    const listbox = root!.root.findByProps({ role: "listbox" });
    expect(listbox.props.className).toContain("is-fixed");
    const options = root!.root.findAllByProps({ role: "option" });
    expect(options).toHaveLength(2);

    await act(async () => {
      options[1].props.onClick();
    });
    expect(onChange).toHaveBeenCalledWith("claude-4");
    expect(root!.root.findAllByProps({ role: "listbox" })).toHaveLength(0);
  });

  it("keeps DropdownMultiSelect open when toggling skills and renders inline hints even when hintPresentation='toast'", async () => {
    const onChange = vi.fn();
    await act(async () => {
      root = create(
        <DropdownMultiSelect
          label="技能"
          selected={["skill-a"]}
          onChange={onChange}
          hintPresentation="toast"
          options={[
            { id: "skill-a", label: "Skill A", hint: "First skill description" },
            { id: "skill-b", label: "Skill B", hint: "Second skill description" },
          ]}
        />,
      );
    });

    const trigger = root!.root.findByProps({ "aria-haspopup": "listbox" });
    await act(async () => {
      trigger.props.onClick();
    });

    const hints = root!.root.findAll(
      (n) => n.props.className === "agentz-dropdown-option-hint",
    );
    expect(hints.map((h) => h.children.join(""))).toEqual([
      "First skill description",
      "Second skill description",
    ]);

    const options = root!.root.findAllByProps({ role: "option" });
    await act(async () => {
      options[1].props.onClick();
    });
    expect(onChange).toHaveBeenCalledWith(["skill-a", "skill-b"]);
    // Menu stays open after toggling multi-select item
    expect(root!.root.findAllByProps({ role: "listbox" })).toHaveLength(1);
  });

  it("does not flip placement='up' downward off-screen when composer is at the bottom of a short window", () => {
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1200,
      innerHeight: 260,
    };

    const trigger = {
      getBoundingClientRect: () => ({
        top: 210,
        bottom: 242,
        left: 100,
        right: 180,
        width: 80,
        height: 32,
      }),
      closest: () => null,
    } as unknown as HTMLElement;

    const popup = {
      scrollHeight: 280,
      offsetHeight: 280,
      offsetWidth: 260,
    } as unknown as HTMLElement;

    const style = computeFixedPopupPosition(trigger, popup, "up", "text");
    // Must stay opening upward (anchored via `bottom`) and clamp maxHeight to available space above
    expect(style.top).toBe("auto");
    expect(style.bottom).toBe(260 - 210 + 6);
    expect(Number(style.maxHeight)).toBeLessThanOrEqual(210 - 6 - 8);
    expect(style.zIndex).toBe(100100);
  });

  it("anchors horizontal position from right when popup right edge exceeds the window", () => {
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1200,
      innerHeight: 800,
    };

    const trigger = {
      getBoundingClientRect: () => ({
        top: 740,
        bottom: 768,
        left: 1110,
        right: 1175,
        width: 65,
        height: 28,
      }),
      closest: () => null,
    } as unknown as HTMLElement;

    const popup = {
      scrollHeight: 240,
      offsetHeight: 240,
      offsetWidth: 300,
    } as unknown as HTMLElement;

    const style = computeFixedPopupPosition(trigger, popup, "up", "text");
    expect(style.left).toBe("auto");
    expect(style.right).toBe(1200 - 1175);
    expect(1200 - Number(style.right) - 300).toBeGreaterThanOrEqual(8);
  });

  it("anchors vertical position from bottom when placement='down' exceeds the bottom of the window", () => {
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1200,
      innerHeight: 600,
    };

    // Case 1: Trigger near bottom — flips above trigger using `bottom` as reference
    const bottomTrigger = {
      getBoundingClientRect: () => ({
        top: 460,
        bottom: 492,
        left: 120,
        right: 280,
        width: 160,
        height: 32,
      }),
      closest: () => null,
    } as unknown as HTMLElement;

    const popup = {
      scrollHeight: 240,
      offsetHeight: 240,
      offsetWidth: 220,
    } as unknown as HTMLElement;

    const flippedStyle = computeFixedPopupPosition(bottomTrigger, popup, "down", "field");
    expect(flippedStyle.top).toBe("auto");
    expect(flippedStyle.bottom).toBe(600 - 460 + 6);

    // Case 2: Trigger near top of a short window where popup exceeds bottom — anchors to window bottom
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1200,
      innerHeight: 300,
    };
    const topTrigger = {
      getBoundingClientRect: () => ({
        top: 8,
        bottom: 36,
        left: 120,
        right: 220,
        width: 100,
        height: 28,
      }),
      closest: () => null,
    } as unknown as HTMLElement;

    const tallPopup = {
      scrollHeight: 360,
      offsetHeight: 360,
      offsetWidth: 260,
    } as unknown as HTMLElement;

    const clampedStyle = computeFixedPopupPosition(topTrigger, tallPopup, "down", "text", {
      maxHeight: 360,
    });
    expect(clampedStyle.top).toBe("auto");
    expect(clampedStyle.bottom).toBe(8);
    expect(Number(clampedStyle.maxHeight)).toBeLessThanOrEqual(300 - 42 - 8);
  });

  it("normalizes coordinates when .agentz-app has CSS zoom scaling", () => {
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1250,
      innerHeight: 1000,
    };

    const appEl = {
      getBoundingClientRect: () => ({
        top: 0,
        bottom: 800,
        left: 0,
        right: 1000,
        width: 1000, // 1250 / 1.25 zoom
        height: 800, // 1000 / 1.25 zoom
      }),
    } as unknown as HTMLElement;

    const trigger = {
      getBoundingClientRect: () => ({
        top: 720, // 900 in viewport coords
        bottom: 744,
        left: 880, // 1100 in viewport coords
        right: 940, // 1175 in viewport coords
        width: 60,
        height: 24,
      }),
      closest: (sel: string) => (sel.includes(".agentz-app") ? appEl : null),
    } as unknown as HTMLElement;

    const popup = {
      offsetWidth: 280,
      scrollHeight: 200,
    } as unknown as HTMLElement;

    const style = computeFixedPopupPosition(trigger, popup, "up", "text");
    // 1000 - (720 * 1.25) + 6 = 106
    expect(style.bottom).toBe(106);
    expect(style.left).toBe("auto");
    expect(style.right).toBe(1250 - 1175);
  });

  it("calculates right-click context menu position from right and bottom when exceeding window bounds", () => {
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1000,
      innerHeight: 700,
    };

    const menuEl = {
      offsetWidth: 220,
      scrollWidth: 220,
      offsetHeight: 280,
      scrollHeight: 280,
    } as unknown as HTMLElement;

    // 1. Inside viewport bounds -> uses top & left
    const normal = computeContextMenuPosition({ x: 150, y: 120 }, menuEl);
    expect(normal.left).toBe(150);
    expect(normal.right).toBe("auto");
    expect(normal.top).toBe(120);
    expect(normal.bottom).toBe("auto");

    // 2. Exceeds right and bottom edges -> anchors via right & bottom
    const corner = computeContextMenuPosition({ x: 860, y: 520 }, menuEl);
    expect(corner.left).toBe("auto");
    expect(corner.right).toBe(1000 - 860);
    expect(corner.top).toBe("auto");
    expect(corner.bottom).toBe(700 - 520);

    // 3. Very short window where flipping above cursor would also exceed top -> pins bottom to window margin (8px)
    (globalThis as { window?: unknown }).window = {
      innerWidth: 1000,
      innerHeight: 360,
    };
    const shortWin = computeContextMenuPosition({ x: 200, y: 200 }, menuEl);
    expect(shortWin.top).toBe("auto");
    expect(shortWin.bottom).toBe(8);
  });

  it("scales editor and terminal font sizes proportionally with UI font scale and base editor font size", () => {
    expect(UI_FONT_SCALES).toContain(1.25);
    expect(UI_FONT_SCALES).toContain(1.5);
    expect(getEffectiveEditorFontSize(1, DEFAULT_EDITOR_FONT_SIZE)).toBe(14);
    expect(getEffectiveEditorFontSize(1.25, DEFAULT_EDITOR_FONT_SIZE)).toBe(18);
    expect(getEffectiveEditorFontSize(1.5, DEFAULT_EDITOR_FONT_SIZE)).toBe(21);
    expect(getEffectiveEditorFontSize(1.25, 18)).toBe(23);
    expect(getEffectiveTerminalFontSize(1, DEFAULT_EDITOR_FONT_SIZE)).toBe(13);
    expect(getEffectiveTerminalFontSize(1.25, DEFAULT_EDITOR_FONT_SIZE)).toBe(16);
  });
});
