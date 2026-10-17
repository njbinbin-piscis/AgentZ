import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
import FirstRunChecklist from "./FirstRunChecklist";

it("offers a direct folder action before a project is open, then enables the example", () => {
  const onOpenFolder = vi.fn();
  const onTryExample = vi.fn();
  let renderer: ReactTestRenderer;
  act(() => {
    renderer = create(
      <FirstRunChecklist projectReady={false} configured={false} firstTaskDone={false}
        onOpenFolder={onOpenFolder} onOpenSettings={vi.fn()} onTryExample={onTryExample} />,
    );
  });
  let buttons = renderer!.root.findAllByType("button");
  expect(buttons.map((button) => button.children.join(""))).toContain("app.openFolder");
  expect(buttons.find((button) => button.children.join("") === "chat.firstRunTryExample")?.props.disabled).toBe(true);
  act(() => buttons.find((button) => button.children.join("") === "app.openFolder")?.props.onClick());
  expect(onOpenFolder).toHaveBeenCalledOnce();
  expect(onTryExample).not.toHaveBeenCalled();

  act(() => {
    renderer!.update(
      <FirstRunChecklist projectReady configured={false} firstTaskDone={false}
        onOpenFolder={onOpenFolder} onOpenSettings={vi.fn()} onTryExample={onTryExample} />,
    );
  });
  buttons = renderer!.root.findAllByType("button");
  expect(buttons.map((button) => button.children.join(""))).not.toContain("app.openFolder");
  act(() => buttons.find((button) => button.children.join("") === "chat.firstRunTryExample")?.props.onClick());
  expect(onTryExample).toHaveBeenCalledOnce();
  act(() => renderer!.unmount());
});
