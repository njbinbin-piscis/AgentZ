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