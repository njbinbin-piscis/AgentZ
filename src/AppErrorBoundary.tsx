import { Component, type ErrorInfo, type ReactNode } from "react";
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
    const zh = typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh");
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
          {zh ? "界面在恢复时出错" : "The window failed while restoring"}
        </h1>
        <p style={{ maxWidth: 560, lineHeight: 1.5, opacity: 0.85 }}>
          {zh
            ? "上次打开的文件或预览让界面崩溃了。可以不恢复那些文件，重新进入项目。"
            : "A file or preview from the last session crashed the window. You can reopen the project without those files."}
        </p>
        <pre style={{ whiteSpace: "pre-wrap", opacity: 0.7 }}>{this.state.error.message}</pre>
        {this.state.stack && (
          <pre style={{ whiteSpace: "pre-wrap", opacity: 0.55, fontSize: 12 }}>{this.state.stack}</pre>
        )}
        <div style={{ display: "flex", gap: 8, marginTop: 16 }}>
          <button type="button" onClick={() => window.location.reload()}>
            {zh ? "重新加载" : "Reload"}
          </button>
          <button
            type="button"
            onClick={() => {
              requestSkipEditorRestore();
              window.location.reload();
            }}
          >
            {zh ? "不恢复已打开的文件" : "Reopen without restored files"}
          </button>
        </div>
      </div>
    );
  }
}
