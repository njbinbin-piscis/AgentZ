import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { botzListBots } from "../../services/tauri/botz";
import { isBotBusy, isRoutineDue, sendToBot } from "./botRuntime";

const TICK_MS = 60_000;

/** Fires due bot routines while the app is open. Renders nothing. */
export default function BotRoutineRunner() {
  const { i18n } = useTranslation();
  useEffect(() => {
    let stopped = false;
    const tick = async () => {
      let bots;
      try {
        bots = await botzListBots();
      } catch {
        return;
      }
      const now = new Date();
      for (const bot of bots) {
        if (stopped) return;
        if (isBotBusy(bot.id)) continue;
        const due = bot.routines.find((r) => isRoutineDue(r, now));
        if (!due) continue;
        // One routine per bot per tick; the rest follow on later ticks.
        void sendToBot({ agentId: bot.id, text: due.prompt, routine: due, preferZh: i18n.language.startsWith("zh") });
      }
    };
    const first = window.setTimeout(() => void tick(), 15_000);
    const timer = window.setInterval(() => void tick(), TICK_MS);
    return () => {
      stopped = true;
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, [i18n.language]);
  return null;
}
