import { useState } from "react";
import { respondPermissionRequest } from "../../services/tauri/interactive";
import "./InteractiveCard.css";

export interface PermissionRequestCard {
  requestId: string;
  toolName: string;
  toolInput: unknown;
  description: string;
}

/** Non-blocking, one-shot approval surface for an Agent tool call. */
export default function PermissionCard({
  request,
  onResolved,
}: {
  request: PermissionRequestCard;
  onResolved: () => void;
}) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const resolve = async (approved: boolean) => {
    setSubmitting(true);
    setError(null);
    try {
      await respondPermissionRequest(request.requestId, approved);
      onResolved();
    } catch (reason) {
      setError(String(reason));
      setSubmitting(false);
    }
  };

  return (
    <section className="interactive-card agentz-permission-card" aria-live="polite">
      <strong>Permission required</strong>
      <p>{request.description}</p>
      <code>{request.toolName}</code>
      <pre className="agentz-permission-input">{JSON.stringify(request.toolInput, null, 2)}</pre>
      {error && <div className="agentz-inline-edit-error">{error}</div>}
      <div className="ic-actions">
        <button type="button" disabled={submitting} onClick={() => void resolve(false)}>
          Deny
        </button>
        <button type="button" className="ic-primary" disabled={submitting} onClick={() => void resolve(true)}>
          Allow once
        </button>
      </div>
    </section>
  );
}
