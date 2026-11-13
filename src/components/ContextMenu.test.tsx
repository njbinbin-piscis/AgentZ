// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ContextMenu, { useContextMenu, type ContextMenuItem } from "./ContextMenu";

vi.mock("./ContextMenu.css", () => ({}));

afterEach(() => cleanup());

function Harness({ items }: { items: ContextMenuItem[] }) {
  const menu = useContextMenu();
  return (
    <div>
      <div data-testid="target" onContextMenu={menu.openWith(items)}>
        right-click me
      </div>
      {menu.element}
    </div>
  );
}

const items = [
  { id: "copy", label: "Copy", onSelect: vi.fn() },
  { id: "sep", label: "", separator: true },
  { id: "paste", label: "Paste", onSelect: vi.fn() },
  { id: "delete", label: "Delete", danger: true, onSelect: vi.fn() },
];

describe("ContextMenu", () => {
  it("opens at the click point and lists actionable items", async () => {
    const user = userEvent.setup();
    render(<Harness items={items} />);
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    const menu = screen.getByRole("menu");
    expect(menu).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Copy" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Delete" }).className).toContain("is-danger");
    expect(screen.getAllByRole("menuitem").length).toBe(3);
  });

  it("invokes onSelect and closes on click", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<Harness items={[{ id: "a", label: "Action", onSelect }]} />);
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    await user.click(screen.getByRole("menuitem", { name: "Action" }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<Harness items={items} />);
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    expect(screen.getByRole("menu")).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("navigates with arrow keys and activates with Enter", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <Harness
        items={[
          { id: "a", label: "First", onSelect: vi.fn() },
          { id: "b", label: "Second", onSelect },
        ]}
      />,
    );
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("skips disabled items during keyboard navigation", async () => {
    const user = userEvent.setup();
    const onSecond = vi.fn();
    render(
      <Harness
        items={[
          { id: "a", label: "Disabled", disabled: true, onSelect: vi.fn() },
          { id: "b", label: "Enabled", onSelect: onSecond },
        ]}
      />,
    );
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    await user.keyboard("{ArrowDown}{Enter}");
    expect(onSecond).toHaveBeenCalledTimes(1);
  });

  it("closes when clicking outside", async () => {
    const user = userEvent.setup();
    render(<Harness items={items} />);
    await user.pointer({ keys: "[MouseRight]", target: screen.getByTestId("target") });
    expect(screen.getByRole("menu")).toBeTruthy();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("renders nothing when state is null", () => {
    render(<ContextMenu state={null} onClose={() => {}} />);
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
