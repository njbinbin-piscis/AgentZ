import { useEffect, useRef, useState } from "react";
import { getPermissionPreview, respondPermissionRequest } from "../../services/tauri/interactive";
import "./InteractiveCard.css";

export interface PermissionRequestCard {
  requestId: string;
  toolName: string;
  toolInput: unknown;
  description: string;
  approval?: {
    digest: string;
    taskId: string;
    environmentId: string;
    expiresMs: number;
    resource?: unknown;
  };
}

/** Non-blocking, one-shot approval surface for an Agent tool call. */
export default function PermissionCard({
  request: signal,
  onResolved,
}: {
  request: PermissionRequestCard;
  onResolved: () => void;
}) {
  const signalKey = `${signal.requestId}:${signal.approval?.digest ?? "legacy"}`;
  const [preview, setPreview] = useState<{ key: string; card: PermissionRequestCard } | null>(null);
  const [previewFailure, setPreviewFailure] = useState<{ key: string; message: string } | null>(null);
  const [previewAttempt, setPreviewAttempt] = useState(0);
  const previewError = previewFailure?.key === signalKey ? previewFailure.message : null;
  const request = signal.requestId.startsWith("approval:")
    ? (preview?.key === signalKey ? preview.card : {
      ...signal, toolName: "", toolInput: null, description: "Loading trusted approval preview", approval: undefined,
    }) : signal;
  useEffect(() => {
    if (!signal.requestId.startsWith("approval:")) return;
    let live = true;
    setPreviewFailure(null);
    void getPermissionPreview(signal.requestId).then(card => {
      if (card.requestId !== signal.requestId || !card.approval?.digest || !Number.isFinite(card.approval.expiresMs)) throw new Error("Invalid host approval preview");
      if (live) setPreview({ key: signalKey, card });
    }).catch(reason => { if (live) setPreviewFailure({ key: signalKey, message: String(reason) }); });
    return () => { live = false; };
  }, [signal.requestId, signalKey, previewAttempt]);
  const previewKey = `${signalKey}:${request.approval?.digest ?? "loading"}`;
  const currentId = useRef(previewKey);
  const submittedKey = useRef<string | null>(null);
  currentId.current = previewKey;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { submittedKey.current = null; setSubmitting(false); setError(null); }, [previewKey]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [clock, setClock] = useState(Date.now());
  const expiresMs = request.approval?.expiresMs;
  const expired = expiresMs !== undefined && (!Number.isFinite(expiresMs) || clock >= expiresMs);
  useEffect(() => {
    setClock(Date.now());
    if (expiresMs === undefined || !Number.isFinite(expiresMs) || expiresMs <= Date.now()) return;
    const timer = setTimeout(() => setClock(Date.now()), Math.min(expiresMs - Date.now(), 2147483647));
    return () => clearTimeout(timer);
  }, [expiresMs, previewKey]);
  const resolve = async (approved: boolean, alwaysAllow = false) => {
    if (!mounted.current || currentId.current !== previewKey) return;
    if (submittedKey.current === previewKey) return;
    if (approved && signal.requestId.startsWith("approval:") && !request.approval?.digest) return;
    if (approved && (expired || (expiresMs !== undefined && (!Number.isFinite(expiresMs) || Date.now() >= expiresMs)))) {
      setClock(Date.now());
      setError(null);
      return;
    }
    const requestId = previewKey;
    submittedKey.current = requestId;
    setSubmitting(true);
    setError(null);
    try {
      if (alwaysAllow) {
        await respondPermissionRequest(request.requestId, approved, request.approval?.digest, true);
      } else {
        await respondPermissionRequest(request.requestId, approved, request.approval?.digest);
      }
      if (mounted.current && currentId.current === requestId) onResolved();
    } catch (reason) {
      if (!mounted.current || currentId.current !== requestId) return;
      submittedKey.current = null;
      setError(String(reason));
      setSubmitting(false);
    }
  };
  const dismiss = () => {
    if (mounted.current && currentId.current === previewKey) onResolved();
  };
  const localDismiss = expired || !!previewError;
  const allowDisabled = submitting || expired || (request.requestId.startsWith("approval:") && !request.approval?.digest);

  return (
    <section className="interactive-card agentz-permission-card" aria-live="polite">
      <strong>Permission required</strong>
      <p>{request.description}</p>
      {request.approval && (
        <p className="agentz-permission-meta">Task: {request.approval.taskId} · Environment: {request.approval.environmentId}
          <br />Expires: {new Date(request.approval.expiresMs).toLocaleString()}
        </p>
      )}
      {request.approval?.resource != null && <pre className="agentz-permission-resource">{JSON.stringify(request.approval.resource, null, 2)}</pre>}
      <code className="agentz-permission-tool">{request.toolName}</code>
      <pre className="agentz-permission-input">{JSON.stringify(request.toolInput, null, 2)}</pre>
      {error && <div className="agentz-inline-edit-error">{error}</div>}
      {previewError && <div role="alert" className="agentz-inline-edit-error">{previewError}</div>}
      {expired && <p role="status">This approval has expired. Request a new operation.</p>}
      <div className="ic-actions">
        <button type="button" className="ic-btn ic-btn-default" disabled={submitting} onClick={() => localDismiss ? dismiss() : void resolve(false)}>
          {localDismiss ? "Dismiss" : "Deny"}
        </button>
        <button type="button" className="ic-btn ic-btn-primary ic-primary" disabled={allowDisabled} onClick={() => void resolve(true)}>
          Allow once
        </button>
        {previewError ? (
          <button type="button" className="ic-btn ic-btn-default" onClick={() => setPreviewAttempt(attempt => attempt + 1)}>Retry preview</button>
        ) : (
          <button type="button" className="ic-btn ic-btn-always-allow" disabled={allowDisabled} onClick={() => void resolve(true, true)}>
            Always allow
          </button>
        )}
      </div>
    </section>
  );
}
