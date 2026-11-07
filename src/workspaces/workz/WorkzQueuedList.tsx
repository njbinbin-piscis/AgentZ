import { useTranslation } from "react-i18next";
import TaskCard from "../../components/TaskCard";

export interface WorkzQueuedListProps {
  items: string[];
  onInterrupt: (index: number) => void;
  onSteer: (index: number) => void;
  onEdit: (index: number) => void;
  onRemove: (index: number) => void;
}

/** Follow-up messages waiting behind the current WorkZ turn. */
export default function WorkzQueuedList({
  items,
  onInterrupt,
  onSteer,
  onEdit,
  onRemove,
}: WorkzQueuedListProps) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  return (
    <>
      {items.map((text, index) => (
        <div key={`q-${index}`} className="agentz-workz-msg user agentz-workz-queue" data-testid="workz-queued">
          <div className="agentz-workz-msg-role">{t("chat.you")}</div>
          <div className="agentz-workz-msg-body">
            <TaskCard text={text} />
            <div className="agentz-queue-actions">
              <button
                type="button"
                className="agentz-queue-action"
                title={t("chat.queueInterruptSend")}
                aria-label={t("chat.queueInterruptSend")}
                onClick={() => onInterrupt(index)}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" aria-hidden>
                  <path d="M4 4h4v16H4zM10 6l10 6-10 6V6z" />
                </svg>
              </button>
              <button
                type="button"
                className="agentz-queue-action"
                title={t("chat.queueSteerNext")}
                aria-label={t("chat.queueSteerNext")}
                onClick={() => onSteer(index)}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
                  <path d="M4 6h9v6H4z" />
                  <path d="M8 16h11" />
                  <path d="M15 12l4 4-4 4" />
                </svg>
              </button>
              <button
                type="button"
                className="agentz-queue-action"
                title={t("chat.queueEditReturn")}
                aria-label={t("chat.queueEditReturn")}
                onClick={() => onEdit(index)}
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
                  <path d="M12 20h9" />
                  <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4 11.5-11.5z" />
                </svg>
              </button>
              <button
                type="button"
                className="agentz-queue-action danger"
                title={t("chat.removeFromQueue")}
                aria-label={t("chat.removeFromQueue")}
                onClick={() => onRemove(index)}
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      ))}
    </>
  );
}
