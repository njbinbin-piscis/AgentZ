import { useCallback, useEffect, useMemo, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import {
  listSlashCommands,
  type SlashCommandInfo,
} from "../services/tauri/slashCommands";

export interface SlashState {
  query: string;
  start: number;
  caret: number;
  active: number;
}

function pickLocalized(zh: string, en: string, lang: string): string {
  if (lang.startsWith("zh") && zh.trim()) return zh;
  return en;
}

export function useSlashCompletion(
  setInput: (fn: (cur: string) => string) => void,
  textareaRef?: RefObject<HTMLTextAreaElement | null>,
) {
  const { i18n } = useTranslation();
  const [slash, setSlash] = useState<SlashState | null>(null);
  const [commands, setCommands] = useState<SlashCommandInfo[]>([]);

  useEffect(() => {
    listSlashCommands()
      .then(setCommands)
      .catch(() => setCommands([]));
  }, []);

  const detectSlash = useCallback((value: string, caret?: number) => {
    const pos = caret ?? value.length;
    const before = value.slice(0, pos);
    const m = before.match(/(?:^|\s)\/([^\s]*)$/);
    if (m) {
      setSlash({ query: m[1], start: pos - m[1].length - 1, caret: pos, active: 0 });
    } else {
      setSlash(null);
    }
  }, []);

  const slashMatches = useMemo(() => {
    if (!slash) return [];
    const q = slash.query.toLowerCase();
    const out: SlashCommandInfo[] = [];
    for (const cmd of commands) {
      if (out.length >= 12) break;
      const id = cmd.slash_id || cmd.id;
      if (
        id.toLowerCase().includes(q) ||
        cmd.name.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q) ||
        cmd.description_zh.toLowerCase().includes(q)
      ) {
        out.push(cmd);
      }
    }
    return out;
  }, [slash, commands]);

  const pickSlash = useCallback(
    (cmd: SlashCommandInfo) => {
      if (!slash) return;
      const id = cmd.slash_id || cmd.id;
      setInput((cur) => {
        const next = `${cur.slice(0, slash.start)}/${id} ${cur.slice(slash.caret)}`;
        const pos = slash.start + id.length + 2;
        requestAnimationFrame(() => {
          textareaRef?.current?.focus();
          textareaRef?.current?.setSelectionRange(pos, pos);
        });
        return next;
      });
      setSlash(null);
    },
    [slash, setInput, textareaRef],
  );

  const slashLabel = useCallback(
    (cmd: SlashCommandInfo) => {
      const id = cmd.slash_id || cmd.id;
      const desc = pickLocalized(cmd.description_zh, cmd.description, i18n.language);
      return desc ? `/${id} — ${desc}` : `/${id}`;
    },
    [i18n.language],
  );

  const handleSlashKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>, onPickDefault?: () => void) => {
      const active =
        slash && slashMatches.length > 0 && (slash.query.length > 0 || slashMatches.length > 1);
      if (!active || !slash) return false;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSlash({ ...slash, active: (slash.active + 1) % slashMatches.length });
        return true;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSlash({
          ...slash,
          active: (slash.active - 1 + slashMatches.length) % slashMatches.length,
        });
        return true;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        pickSlash(slashMatches[slash.active]);
        onPickDefault?.();
        return true;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setSlash(null);
        return true;
      }
      return false;
    },
    [slash, slashMatches, pickSlash],
  );

  return {
    slash,
    setSlash,
    slashMatches,
    pickSlash,
    slashLabel,
    detectSlash,
    handleSlashKeyDown,
  };
}
