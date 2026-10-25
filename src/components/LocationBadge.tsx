import { useTranslation } from "react-i18next";
import { CAPABILITIES, capabilitiesFor, describeRemoteDir, isRemoteDir } from "../extensions/remoteTargets";

/** Where the current project runs, with the capability table as tooltip. */
export default function LocationBadge({ dir }: { dir: string | null }) {
  const { t } = useTranslation();
  if (!dir) return null;
  const remote = isRemoteDir(dir);
  const caps = capabilitiesFor(dir);
  const label = remote ? describeRemoteDir(dir) ?? t("remote.locationRemote") : t("remote.locationLocal");
  const title = [
    remote ? t("remote.runsRemote", { where: label }) : t("remote.runsLocal"),
    ...CAPABILITIES.map((c) => `${caps[c] ? "✅" : "❌"} ${t(`remote.cap_${c}`)}`),
  ].join("\n");
  return (
    <span className={`agentz-location-badge ${remote ? "remote" : "local"}`} title={title}>
      {remote ? "🔌" : "💻"} {label}
    </span>
  );
}

/** Shown inside WorkZ / BotZ when the active project is remote. */
export function RemoteBoundaryBanner({
  dir,
  feature,
  onOpenLocal,
  onBackToCodeZ,
}: {
  dir: string | null;
  feature: "teams" | "rooms";
  onOpenLocal: () => void;
  onBackToCodeZ: () => void;
}) {
  const { t } = useTranslation();
  if (!dir || capabilitiesFor(dir)[feature]) return null;
  const where = describeRemoteDir(dir) ?? dir;
  return (
    <div className="agentz-remote-boundary" role="status">
      <span>🔌 {t(feature === "teams" ? "remote.boundaryTeams" : "remote.boundaryRooms", { where })}</span>
      <button type="button" onClick={onOpenLocal}>
        {t("remote.openLocal")}
      </button>
      <button type="button" onClick={onBackToCodeZ}>
        {t("remote.backToCodeZ")}
      </button>
    </div>
  );
}
