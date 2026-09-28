import { useEffect, useRef, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { loadMermaid, sanitizeMermaidSvg } from "./mermaidSafe";
import "highlight.js/styles/github-dark.css";
import "./MarkdownPreview.css";

interface MarkdownPreviewProps {
  content: string;
}

function MermaidBlock({ code }: { code: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const id = useMemo(() => `mmd-${Math.random().toString(36).slice(2)}`, []);

  useEffect(() => {
    let cancelled = false;
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
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [code, id]);

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
