import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { KoiTodo } from "../../services/tauri/pool";
import { botzReviewTodo, botzTodoDiff, type TodoDiff } from "../../services/tauri/botz";

export interface DiffLine {
  kind: "file" | "hunk" | "add" | "remove" | "context" | "meta";
  text: string;
  path?: string;
  /** Line number in the new file for add/context lines. */
  line?: number;
}

/** Parse `git diff` output into displayable lines with new-file line numbers. */
export function parseUnifiedDiff(diff: string): DiffLine[] {
  const out: DiffLine[] = [];
  let path: string | undefined;
  let newLine = 0;
  for (const raw of diff.split("\n")) {
    if (raw.startsWith("diff --git ")) {
      const m = / b\/(.+)$/.exec(raw);
      path = m?.[1];
      out.push({ kind: "file", text: path ?? raw, path });
    } else if (raw.startsWith("@@")) {
      const m = /\+(\d+)/.exec(raw);
      newLine = m ? Number(m[1]) : 0;
      out.push({ kind: "hunk", text: raw, path });
    } else if (raw.startsWith("+++") || raw.startsWith("---") || raw.startsWith("index ") || raw.startsWith("new file") || raw.startsWith("deleted file")) {
      out.push({ kind: "meta", text: raw, path });
    } else if (raw.startsWith("+")) {
      out.push({ kind: "add", text: raw.slice(1), path, line: newLine++ });
    } else if (raw.startsWith("-")) {
      out.push({ kind: "remove", text: raw.slice(1), path });
    } else if (raw.startsWith(" ")) {
      out.push({ kind: "context", text: raw.slice(1), path, line: newLine++ });
    }
  }
  return out;
}

const MAX_ROWS = 3000;

export default function ReviewPanel({
  projectDir,
  todo,
  ownerName,
  onClose,
  onDone,
}: {
  projectDir: string;
  todo: KoiTodo;
  ownerName: string;
  onClose: () => void;
  onDone: (message: string) => void;
}) {
  const { t } = useTranslation();
  const [diff, setDiff] = useState<TodoDiff | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    botzTodoDiff(projectDir, todo.id).then(setDiff, (e) => setError(String(e)));
  }, [projectDir, todo.id]);

  const lines = useMemo(() => (diff ? parseUnifiedDiff(diff.diff) : []), [diff]);

  const review = async (approve: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const res = await botzReviewTodo(projectDir, todo.id, approve, feedback);
      onDone(res.message);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="botz-review-overlay" onClick={onClose}>
      <div className="botz-review" onClick={(e) => e.stopPropagation()}>
        <div className="botz-drawer-head">
          <strong>{todo.title}</strong>
          <span className="botz-drawer-role">
            {ownerName}
            {diff?.branch ? ` · ${diff.branch}` : ""}
          </span>
          <button type="button" onClick={onClose} title={t("common.close")}>
            ✕
          </button>
        </div>
        {todo.description && <div className="botz-review-desc">{todo.description}</div>}
        <div className="botz-review-diff">
          {!diff && !error && <div className="botz-empty">{t("botz.loadingDiff")}</div>}
          {diff && !diff.branch && <div className="botz-empty">{t("botz.noBranch")}</div>}
          {diff?.branch && lines.length === 0 && <div className="botz-empty">{t("botz.noChanges")}</div>}
          {lines.slice(0, MAX_ROWS).map((l, i) => (
            <div
              key={i}
              className={`botz-diff-line ${l.kind}`}
              title={l.path && l.line ? t("botz.quoteLine") : undefined}
              onClick={() => {
                if (!l.path || l.kind === "file" || l.kind === "meta") return;
                const ref = l.line ? `${l.path}:${l.line}` : l.path;
                setFeedback((f) => `${f}${f && !f.endsWith("\n") ? "\n" : ""}${ref} \`${l.text.trim().slice(0, 80)}\` — `);
              }}
            >
              {(l.kind === "add" || l.kind === "context") && <span className="botz-diff-no">{l.line}</span>}
              {l.kind === "remove" && <span className="botz-diff-no" />}
              <span className="botz-diff-text">
                {l.kind === "add" ? "+" : l.kind === "remove" ? "-" : l.kind === "context" ? " " : ""}
                {l.text}
              </span>
            </div>
          ))}
          {(diff?.truncated || lines.length > MAX_ROWS) && <div className="botz-empty">{t("botz.diffTruncated")}</div>}
        </div>
        {error && <div className="botz-error">{error}</div>}
        <div className="botz-review-actions">
          <textarea value={feedback} onChange={(e) => setFeedback(e.target.value)} placeholder={t("botz.feedbackPlaceholder")} />
          <div className="botz-review-buttons">
            <button type="button" disabled={busy} onClick={() => void review(false)}>
              {t("botz.requestChanges")}
            </button>
            <button type="button" className="primary" disabled={busy} onClick={() => void review(true)}>
              {diff?.branch ? t("botz.approveMerge") : t("botz.approve")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
