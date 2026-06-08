import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../store";

interface Props {
  autorunTotal: number;
  autorunFiltered: number;
  networkTotal: number;
  networkFiltered: number;
  eventTotal: number;
  eventFiltered: number;
}

export function WorkspaceStatsBar({
  autorunTotal,
  autorunFiltered,
  networkTotal,
  networkFiltered,
  eventTotal,
  eventFiltered,
}: Props) {
  const { t } = useTranslation();
  const scanning = useWorkspaceStore((s) => s.scanning);

  return (
    <div className="h-7 px-3 flex items-center gap-4 bg-bg-elev-1 border-t border-border text-xs text-fg-secondary">
      <span>
        {t("workspace.stats.autoruns", { filtered: autorunFiltered, total: autorunTotal })}
      </span>
      <span>
        {t("workspace.stats.network", { filtered: networkFiltered, total: networkTotal })}
      </span>
      <span>
        {t("workspace.stats.events", { filtered: eventFiltered, total: eventTotal })}
      </span>
      <div className="flex-1" />
      {scanning && (
        <span className="text-accent animate-pulse">扫描中…</span>
      )}
    </div>
  );
}
