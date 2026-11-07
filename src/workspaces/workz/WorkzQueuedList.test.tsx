import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";
import WorkzQueuedList from "./WorkzQueuedList";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("WorkzQueuedList", () => {
  it("offers interrupt, steer, edit, and remove for each queued message", async () => {
    const onInterrupt = vi.fn();
    const onSteer = vi.fn();
    const onEdit = vi.fn();
    const onRemove = vi.fn();
    let root!: ReactTestRenderer;
    await act(async () => {
      root = create(
        <WorkzQueuedList
          items={["watch the failing test"]}
          onInterrupt={onInterrupt}
          onSteer={onSteer}
          onEdit={onEdit}
          onRemove={onRemove}
        />,
      );
    });
    const click = (label: string) => root.root.findByProps({ "aria-label": label }).props.onClick();
    click("chat.queueInterruptSend");
    click("chat.queueSteerNext");
    click("chat.queueEditReturn");
    click("chat.removeFromQueue");
    expect(onInterrupt).toHaveBeenCalledWith(0);
    expect(onSteer).toHaveBeenCalledWith(0);
    expect(onEdit).toHaveBeenCalledWith(0);
    expect(onRemove).toHaveBeenCalledWith(0);
    expect(JSON.stringify(root.toJSON())).toContain("watch the failing test");
  });
});
