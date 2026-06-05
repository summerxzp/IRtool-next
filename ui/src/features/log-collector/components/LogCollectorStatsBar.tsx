import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS } from "../types";
import type { SysmonEventType } from "../types";

interface Props {
  sysmonRunning: boolean;
}

function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

export function LogCollectorStatsBar({ sysmonRunning }: Props) {
  const { t } = useTranslation();
  const { events, collecting, startTime } = useLogCollectorStore();
  const [elapsed, setElapsed] = useState("");

  useEffect(() => {
    if (!collecting || !startTime) { setElapsed(""); return; }
    const update = () => setElapsed(formatDuration(Date.now() - startTime));
    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, [collecting, startTime]);

  const counts = events.reduce((acc, e) => {
    const type = e.event_type;
    acc[type] = (acc[type] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 bg-bg-elev-1 border-t border-border text-xs text-fg-secondary">
      {collecting ? (
        <Badge variant="outline" className="text-[10px] px-1.5 py-0 bg-green-500/15 text-green-500 border-green-500/25">
          {t("log-collector.stats.collecting")}
        </Badge>
      ) : (
        <Badge variant="outline" className="text-[10px] px-1.5 py-0 bg-gray-500/15 text-gray-500 border-gray-500/25">
          {t("log-collector.stats.stopped")}
        </Badge>
      )}

      <Separator orientation="vertical" className="h-3" />

      <span>{t("log-collector.stats.events")}: {events.length.toLocaleString()}</span>

      {Object.entries(counts).map(([type, count]) => (
        <span key={type} className="text-fg-tertiary">
          {EVENT_TYPE_LABELS[type as SysmonEventType] || type}: {count}
        </span>
      ))}

      {elapsed && (
        <>
          <Separator orientation="vertical" className="h-3" />
          <span>{t("log-collector.stats.duration")}: {elapsed}</span>
        </>
      )}

      <div className="flex-1" />

      <span className="text-fg-tertiary">
        Sysmon: {sysmonRunning ? t("log-collector.stats.running") : t("log-collector.stats.not-running")}
      </span>
    </div>
  );
}
