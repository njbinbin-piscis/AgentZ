import { Component, type ErrorInfo, type ReactNode } from "react";
import { appErrorCopy } from "./appErrorCopy";
import { requestSkipEditorRestore } from "./workspaces/codez/editorRestoreGuard";

interface State {
  error: Error | null;
  stack: string;
}

/** Last resort when a restored file crashes the tree and leaves #root empty. */
export default class AppErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null, stack: "" };

  static getDerivedStateFromError(error: Error): Pick<State, "error"> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[AgentZ] window render failed", error, info.componentStack);
    this.setState({ stack: info.componentStack ?? "" });
  }

  render() {
    if (!this.state.error) return this.props.children;
    const copy = appErrorCopy(typeof navigator !== "undefined" ? navigator.language : "en");
    return (
      <div
        style={{
          boxSizing: "border-box",
          minHeight: "100vh",
          padding: 32,
          background: "#0e0e14",
          color: "#e8e8f0",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <h1 style={{ fontSize: 20, margin: "0 0 12px" }}>
          {copy.title}
        </h1>
        <p style={{ maxWidth: 560, lineHeight: 1.5, opacity: 0.85 }}>
          {copy.body}
        </p>
        <pre style={{ whiteSpace: "pre-wrap", opacity: 0.7 }}>{this.state.error.message}</pre>
        {this.state.stack && (
          <pre style={{ whiteSpace: "pre-wrap", opacity: 0.55, fontSize: 12 }}>{this.state.stack}</pre>
        )}
        <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
          <button type="button" onClick={() => window.location.reload()}>
            {copy.reload}
          </button>
          <button
            type="button"
            onClick={() => {
              requestSkipEditorRestore();
              window.location.reload();
            }}
          >
            {copy.skip}
          </button>
        </div>
      </div>
    );
  }
}
