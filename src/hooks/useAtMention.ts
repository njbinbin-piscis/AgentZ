import { useCallback, useMemo, useState, type RefObject } from "react";

export const CODEBASE_MENTION = "codebase";
export const GRAPH_MENTION = "graph";

export interface MentionState {
  query: string;
  start: number;
  caret: number;
  active: number;
}

/**
 * @-mention completion for agent chat composers.
 * Surfaces `@codebase` / `@graph` plus optional project file paths.
 */
export function useAtMention(
  setInput: (fn: (cur: string) => string) => void,
  textareaRef?: RefObject<HTMLTextAreaElement | null>,
  filePaths: string[] = [],
) {
  const [mention, setMention] = useState<MentionState | null>(null);

  const detectAtMention = useCallback((value: string, caret?: number) => {
    const pos = caret ?? value.length;
    const before = value.slice(0, pos);
    const m = before.match(/(?:^|\s)@([^\s]*)$/);
    if (m) {
      setMention({ query: m[1], start: pos - m[1].length - 1, caret: pos, active: 0 });
      return true;
    }
    setMention(null);
    return false;
  }, []);

  const mentionMatches = useMemo(() => {
    if (!mention) return [];
    const q = mention.query.toLowerCase();
    const out: string[] = [];
    if (CODEBASE_MENTION.startsWith(q)) out.push(CODEBASE_MENTION);
    if (GRAPH_MENTION.startsWith(q)) out.push(GRAPH_MENTION);
    for (const f of filePaths) {
      if (out.length >= 9) break;
      if (f.toLowerCase().includes(q)) out.push(f);
    }
    return out;
  }, [mention, filePaths]);

  const pickMention = useCallback(
    (path: string) => {
      if (!mention) return;
      setInput((cur) => {
        const next = cur.slice(0, mention.start) + "@" + path + " " + cur.slice(mention.caret);
        const pos = mention.start + path.length + 2;
        requestAnimationFrame(() => {
          textareaRef?.current?.focus();
          textareaRef?.current?.setSelectionRange(pos, pos);
        });
        return next;
      });
      setMention(null);
    },
    [mention, setInput, textareaRef],
  );

  const handleMentionKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      const active = mention && mentionMatches.length > 0 && mention.query.length > 0;
      if (!active || !mention) return false;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMention({ ...mention, active: (mention.active + 1) % mentionMatches.length });
        return true;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setMention({
          ...mention,
          active: (mention.active - 1 + mentionMatches.length) % mentionMatches.length,
        });
        return true;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        pickMention(mentionMatches[mention.active]!);
        return true;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setMention(null);
        return true;
      }
      return false;
    },
    [mention, mentionMatches, pickMention],
  );

  return {
    mention,
    setMention,
    mentionMatches,
    pickMention,
    detectAtMention,
    handleMentionKeyDown,
  };
}
