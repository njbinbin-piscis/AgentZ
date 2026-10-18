import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { loadMermaid, sanitizeMermaidSvg } from "./mermaidSafe";
import "highlight.js/styles/github-dark.css";
import "../../components/hljs-light.css";
import "./MarkdownPreview.css";

interface MarkdownPreviewProps {
  content: string;
}

let previewMermaidCounter = 0;

function MermaidBlock({ code }: { code: string }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    const id = `mmd-preview-${++previewMermaidCounter}`;
    ref.current?.classList.remove("agentz-md-mermaid-error");
    (async () => {
      try {
        const mermaid = await loadMermaid();
        const { svg } = await mermaid.render(id, code);
        if (!cancelled && ref.current) ref.current.innerHTML = sanitizeMermaidSvg(svg);
      } catch (e) {
        if (!cancelled && ref.current) {
          ref.current.textContent = String(e);
          ref.current.classList.add("agentz-md-mermaid-error");
        }
      } finally {
        document.getElementById(`d${id}`)?.remove();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [code]);

  return <div className="agentz-md-mermaid" ref={ref} />;
}

export default function MarkdownPreview({ content }: MarkdownPreviewProps) {
  return (
    <div className="agentz-md-preview">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={{
          code({ className, children, ...props }) {
            const text = String(children).replace(/\n$/, "");
            const match = /language-(\w+)/.exec(className || "");
            const lang = match?.[1]?.toLowerCase();

            if (lang === "mermaid") {
              return <MermaidBlock code={text} />;
            }

            const isBlock = className?.includes("language-") || text.includes("\n");
            if (isBlock) {
              return (
                <pre className={className}>
                  <code {...props}>{text}</code>
                </pre>
              );
            }
            return (
              <code className="agentz-md-inline-code" {...props}>
                {children}
              </code>
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
