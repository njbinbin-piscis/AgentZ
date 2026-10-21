import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import DropdownSelect, {
  DropdownMultiSelect,
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

  it("clamps horizontal position inside the right window edge for CodeZ right-panel composer menus", () => {
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
    expect(Number(style.left) + 300).toBeLessThanOrEqual(1200 - 8);
    expect(Number(style.left)).toBeGreaterThanOrEqual(8);
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
      closest: (sel: string) => (sel === ".agentz-app" ? appEl : null),
    } as unknown as HTMLElement;

    const popup = {
      offsetWidth: 280,
      scrollHeight: 200,
    } as unknown as HTMLElement;

    const style = computeFixedPopupPosition(trigger, popup, "up", "text");
    // 1000 - (720 * 1.25) + 6 = 106
    expect(style.bottom).toBe(106);
    expect(Number(style.left) + 280).toBeLessThanOrEqual(1250 - 8);
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
