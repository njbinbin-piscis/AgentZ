import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import ChatComposer from "./ChatComposer";
import { createImageAttachmentChip } from "./composerChips";
import type { ChatAttachment } from "../services/tauri/chat";

const tFn = (key: string) => key;
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: tFn }) }));
vi.mock("./DropdownSelect", () => ({
  default: () => null,
  DropdownMultiSelect: () => null,
}));
vi.mock("../utils/popupPosition", () => ({
  canPortalToBody: () => false,
  useFixedPopupPosition: () => ({ style: {}, ref: { current: null } }),
}));
vi.mock("./ChatComposer.css", () => ({}));

const attachment: ChatAttachment = {
  media_type: "image/png",
  data: "aGVsbG8=",
  filename: "shot.png",
};

function renderComposer(busy: boolean, onRemoveChip: (id: string) => void) {
  const chip = createImageAttachmentChip(attachment, "data:image/png;base64,aGVsbG8=");
  return create(
    <ChatComposer
      value="editing a queued message"
      onChange={() => {}}
      onSubmit={() => {}}
      onStop={() => {}}
      busy={busy}
      sendWhileBusy
      placeholder="type"
      modelId="m"
      modelOptions={[]}
      onModelChange={() => {}}
      attachment={null}
      attachmentPreview={null}
      onAttach={() => {}}
      onClearAttachment={() => {}}
      chips={[chip]}
      onRemoveChip={onRemoveChip}
    />,
  );
}

describe("ChatComposer stop button", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  const renderBusy = async (props: { value: string; stopping: boolean; onStop: () => void; onSubmit: () => void }) => {
    await act(async () => {
      root = create(
        <ChatComposer
          value={props.value}
          onChange={() => {}}
          onSubmit={props.onSubmit}
          onStop={props.onStop}
          busy
          stopping={props.stopping}
          stopTitle="stop"
          stoppingTitle="stopping"
          sendWhileBusy
          placeholder="type"
          modelId="m"
          modelOptions={[]}
          onModelChange={() => {}}
          attachment={null}
          attachmentPreview={null}
          onAttach={() => {}}
          onClearAttachment={() => {}}
        />,
      );
    });
    const byClass = (cls: string) =>
      root!.root.findAll((n) => n.type === "button" && String(n.props.className ?? "").split(" ").includes(cls));
    return { stop: byClass("agentz-composer-stop-icon"), send: byClass("agentz-composer-send-busy") };
  };

  it("is clickable while the turn runs", async () => {
    const onStop = vi.fn();
    const { stop } = await renderBusy({ value: "", stopping: false, onStop, onSubmit: () => {} });
    expect(stop).toHaveLength(1);
    expect(stop[0].props.disabled).toBe(false);
    expect(stop[0].props.title).toBe("stop");
    await act(async () => {
      stop[0].props.onClick();
    });
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("stays visible but inert once a stop is pending", async () => {
    const { stop } = await renderBusy({ value: "", stopping: true, onStop: () => {}, onSubmit: () => {} });
    expect(stop).toHaveLength(1);
    expect(stop[0].props.disabled).toBe(true);
    expect(stop[0].props.title).toBe("stopping");
  });

  it("still lets a typed message be queued while stopping", async () => {
    const onSubmit = vi.fn();
    const { stop, send } = await renderBusy({ value: "next", stopping: true, onStop: () => {}, onSubmit });
    expect(stop).toHaveLength(0);
    expect(send).toHaveLength(1);
    expect(send[0].props.disabled).toBeFalsy();
    await act(async () => {
      send[0].props.onClick();
    });
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
});

describe("ChatComposer image chip removal while editing a queued message", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("shows the remove button and removes the image even while a turn is running (busy)", async () => {
    const onRemoveChip = vi.fn();
    await act(async () => {
      root = renderComposer(true, onRemoveChip);
    });
    const removeButtons = root!.root.findAll(
      (n) => n.type === "button" && n.props.className === "agentz-composer-chip-remove",
    );
    expect(removeButtons.length).toBe(1);
    await act(async () => {
      removeButtons[0].props.onClick();
    });
    expect(onRemoveChip).toHaveBeenCalledTimes(1);
  });

  it("still removes the image when not busy", async () => {
    const onRemoveChip = vi.fn();
    await act(async () => {
      root = renderComposer(false, onRemoveChip);
    });
    const removeButtons = root!.root.findAll(
      (n) => n.type === "button" && n.props.className === "agentz-composer-chip-remove",
    );
    expect(removeButtons.length).toBe(1);
    await act(async () => {
      removeButtons[0].props.onClick();
    });
    expect(onRemoveChip).toHaveBeenCalledTimes(1);
  });
});