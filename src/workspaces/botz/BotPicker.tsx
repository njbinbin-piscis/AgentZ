import { useTranslation } from "react-i18next";
import type { AgentInfo } from "../../services/tauri/agents";
import Avatar from "./Avatar";

/** Multi-select of bots (agent ids), shown as avatar chips. */
export default function BotPicker({
  agents,
  selected,
  onChange,
}: {
  agents: AgentInfo[];
  selected: string[];
  onChange: (ids: string[]) => void;
}) {
  const { t } = useTranslation();
  if (agents.length === 0) return <div className="botz-empty">{t("botz.noBots")}</div>;
  const toggle = (id: string) =>
    onChange(selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id]);
  return (
    <div className="botz-bot-picker">
      {agents.map((a) => {
        const on = selected.includes(a.id);
        return (
          <button
            type="button"
            key={a.id}
            className={`botz-bot-chip ${on ? "on" : ""}`}
            aria-pressed={on}
            title={a.role || a.description}
            onClick={() => toggle(a.id)}
          >
            <Avatar seed={a.koi_id ?? a.id} color={a.color} icon={a.icon} activity="idle" />
            <span>{a.name}</span>
          </button>
        );
      })}
    </div>
  );
}
