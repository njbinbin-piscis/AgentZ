import { useTranslation } from "react-i18next";
import { useExtensionUi } from "./useExtensionUi";

/** Renders vscode.window.show{Info,Warning,Error}Message toasts. */
export default function MessageToasts() {
  const { t } = useTranslation();
  const { messages } = useExtensionUi();
  const severityLabel: Record<number, string> = {
    0: t("common.error"),
    1: t("common.warning"),
    2: t("common.info"),
  };
  if (messages.length === 0) return null;
  return (
    <div className="agentz-ext-toasts">
      {messages.map((m) => (
        <div key={m.id} className={`agentz-ext-toast sev-${m.severity}`}>
          <div className="agentz-ext-toast-row">
            <span className="agentz-ext-toast-badge">{severityLabel[m.severity] ?? t("common.info")}</span>
            <span className="agentz-ext-toast-msg">{m.message}</span>
            <button className="agentz-ext-toast-x" onClick={() => m.resolve(undefined)} title={t("common.dismiss")}>
              ✕
            </button>
          </div>
          {m.detail && <div className="agentz-ext-toast-detail">{m.detail}</div>}
          {m.items.length > 0 && (
            <div className="agentz-ext-toast-actions">
              {m.items.map((item) => (
                <button key={item} onClick={() => m.resolve(item)}>
                  {item}
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
