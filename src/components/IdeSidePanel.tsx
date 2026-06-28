import AssistantPanel from "../workspaces/codez/AssistantPanel";
import type { ChatAttachment } from "../services/tauri/chat";
import type { PickedElement } from "../services/tauri/browser";
import "./IdeSidePanel.css";

interface IdeSidePanelProps {
  projectDir: string | null;
  insertRequest?: { paths: string[]; nonce: number } | null;
  insertElementRequest?: { element: PickedElement; nonce: number } | null;
  insertTerminalRequest?: { snippetId: string; text: string; nonce: number } | null;
  attachRequest?: { attachment: ChatAttachment; preview: string | null; nonce: number } | null;
  onAttachRequestHandled?: () => void;
}

export default function IdeSidePanel({
  projectDir,
  insertRequest,
  insertElementRequest,
  insertTerminalRequest,
  attachRequest,
  onAttachRequestHandled,
}: IdeSidePanelProps) {
  return (
    <div className="ide-side-panel">
      <AssistantPanel
        projectDir={projectDir}
        insertRequest={insertRequest}
        insertElementRequest={insertElementRequest}
        insertTerminalRequest={insertTerminalRequest}
        attachRequest={attachRequest}
        onAttachRequestHandled={onAttachRequestHandled}
      />
    </div>
  );
}
