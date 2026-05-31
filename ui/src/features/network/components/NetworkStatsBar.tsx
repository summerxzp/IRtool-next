import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { NetConn } from "../types";

interface Props {
  data: NetConn[];
}

export function NetworkStatsBar({ data }: Props) {
  const { t } = useTranslation();
  const stats = useMemo(() => {
    let endpoints = 0;
    let established = 0;
    let listening = 0;
    let timeWait = 0;
    let closeWait = 0;
    let history = 0;
    for (const c of data) {
      endpoints++;
      if (!c.is_current) {
        history++;
        continue;
      }
      switch (c.state) {
        case "ESTABLISHED": established++; break;
        case "LISTEN": listening++; break;
        case "TIME_WAIT": timeWait++; break;
        case "CLOSE_WAIT": closeWait++; break;
      }
    }
    return { endpoints, established, listening, timeWait, closeWait, history };
  }, [data]);

  return (
    <div className="h-7 px-3 flex items-center gap-4 bg-bg-elev-1 border-t border-border text-xs text-fg-secondary">
      <span>
        {t("network.stats.endpoints")}: <span className="text-fg-primary font-medium">{stats.endpoints}</span>
      </span>
      <span className="text-success">
        {t("network.stats.established")}: <span className="font-medium">{stats.established}</span>
      </span>
      <span className="text-accent">
        {t("network.stats.listening")}: <span className="font-medium">{stats.listening}</span>
      </span>
      <span className="text-warning">
        {t("network.stats.time-wait")}: <span className="font-medium">{stats.timeWait}</span>
      </span>
      <span className="text-danger">
        {t("network.stats.close-wait")}: <span className="font-medium">{stats.closeWait}</span>
      </span>
      <div className="flex-1" />
      <span className="text-fg-tertiary">
        {t("network.stats.history")}: <span className="font-medium">{stats.history}</span>
      </span>
    </div>
  );
}
