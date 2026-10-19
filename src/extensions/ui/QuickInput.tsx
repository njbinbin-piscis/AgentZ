import { useEffect, useRef, useState } from "react";
import { useExtensionUi } from "./useExtensionUi";
import { formatCodiconText } from "../../workspaces/codez/IdeStatusBar";

/** Renders vscode.window.showQuickPick / showInputBox overlays. */
export default function QuickInput() {
  const { quickPick, inputBox } = useExtensionUi();
  const [filter, setFilter] = useState("");
  const [text, setText] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const [checked, setChecked] = useState<Set<number>>(new Set());

  useEffect(() => {
    setFilter("");
    setChecked(new Set());
    inputRef.current?.focus();
  }, [quickPick?.id]);
  useEffect(() => {
    setText(inputBox?.value ?? "");
    inputRef.current?.focus();
  }, [inputBox?.id]);

  if (!quickPick && !inputBox) return null;

  if (inputBox) {
    return (
      <div className="agentz-quick-overlay" onClick={() => inputBox.resolve(undefined)}>
        <div className="agentz-quick" onClick={(e) => e.stopPropagation()}>
          {inputBox.prompt && <div className="agentz-quick-prompt">{inputBox.prompt}</div>}
          <input
            ref={inputRef}
            type={inputBox.password ? "password" : "text"}
            placeholder={inputBox.placeHolder}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") inputBox.resolve(text);
              else if (e.key === "Escape") inputBox.resolve(undefined);
            }}
          />
        </div>
      </div>
    );
  }

  const qp = quickPick!;
  const items = qp.items
    .map((it, idx) => ({ it, idx }))
    .filter(({ it }) => {
      if (!filter.trim()) return true;
      if (it.kind === -1) return false;
      const q = filter.toLowerCase();
      return (
        formatCodiconText(it.label).toLowerCase().includes(q) ||
        (it.description?.toLowerCase().includes(q) ?? false) ||
        (it.detail?.toLowerCase().includes(q) ?? false)
      );
    });

  const accept = () => {
    if (qp.canPickMany) {
      qp.resolve(qp.items.filter((_, i) => checked.has(i)));
    }
  };

  return (
    <div className="agentz-quick-overlay" onClick={() => qp.resolve(undefined)}>
      <div className="agentz-quick" onClick={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          type="text"
          placeholder={qp.placeHolder ?? "Type to filter…"}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") qp.resolve(undefined);
            else if (e.key === "Enter") {
              if (qp.canPickMany) accept();
              else {
                const firstSelectable = items.find(({ it }) => it.kind !== -1);
                if (firstSelectable) qp.resolve(firstSelectable.it);
              }
            }
          }}
        />
        <div className="agentz-quick-list">
          {items.map(({ it, idx }) => {
            if (it.kind === -1) {
              return (
                <div
                  key={idx}
                  className="agentz-quick-separator"
                  style={{
                    padding: "6px 10px 2px",
                    fontSize: 11,
                    fontWeight: 600,
                    opacity: 0.6,
                    textTransform: "uppercase",
                  }}
                >
                  {formatCodiconText(it.label)}
                </div>
              );
            }
            return (
              <button
                key={idx}
                type="button"
                className="agentz-quick-item"
                onClick={() => {
                  if (qp.canPickMany) {
                    setChecked((prev) => {
                      const next = new Set(prev);
                      if (next.has(idx)) next.delete(idx);
                      else next.add(idx);
                      return next;
                    });
                  } else {
                    qp.resolve(it);
                  }
                }}
              >
                {qp.canPickMany && <span className="agentz-quick-check">{checked.has(idx) ? "☑" : "☐"}</span>}
                <span className="agentz-quick-label">{formatCodiconText(it.label)}</span>
                {it.description && <span className="agentz-quick-desc">{formatCodiconText(it.description)}</span>}
                {it.detail && (
                  <span className="agentz-quick-detail" style={{ display: "block", fontSize: 11, opacity: 0.7 }}>
                    {formatCodiconText(it.detail)}
                  </span>
                )}
              </button>
            );
          })}
          {items.length === 0 && <div className="agentz-quick-empty">No matching items</div>}
        </div>
        {qp.canPickMany && (
          <div className="agentz-quick-footer">
            <button type="button" onClick={accept}>OK ({checked.size})</button>
          </div>
        )}
      </div>
    </div>
  );
}
