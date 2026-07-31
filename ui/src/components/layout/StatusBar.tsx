import { useEffect, useState } from "react";
import { commands, type AppInfo, type SysmonStatus } from "@/lib/bindings";
import { useTranslation } from "react-i18next";
import { Shield, ShieldOff, Clock, Activity, Link2, Link2Off, Search, SearchX } from "lucide-react";
import { useNetworkStore } from "@/features/network/store";
import { useLogCollectorStore } from "@/features/log-collector/store";
import { useQueryClient } from "@tanstack/react-query";
import { formatEpochMillis } from "@/lib/utils";
import type { AutorunItem } from "@/features/autoruns/types";

export function StatusBar() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [sysmon, setSysmon] = useState<SysmonStatus | null>(null);
  const [now, setNow] = useState(new Date());

  // Network: paused = false means monitoring
  const networkPaused = useNetworkStore((s) => s.paused);
  // LogCollector: collecting = true means subscribing
  const collecting = useLogCollectorStore((s) => s.collecting);
  // Autoruns: check if scan data exists
  const qc = useQueryClient();
  const [autorunsScanned, setAutorunsScanned] = useState(false);
  useEffect(() => {
    const check = () => {
      const data = qc.getQueryData<AutorunItem[]>(["autoruns", "items"]);
      setAutorunsScanned(!!data && data.length > 0);
    };
    check();
    const id = setInterval(check, 2000);
    return () => clearInterval(id);
  }, [qc]);

  useEffect(() => {
    commands.cmdAppInfo().then(setInfo).catch(() => null);
    commands.cmdSysmonStatus().then((r) => {
      if (r.status === "ok") setSysmon(r.data);
    }).catch(() => null);
  }, []);

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <footer className="h-6 bg-bg-elev-1 border-t border-border flex items-center px-3 gap-3 text-xs text-fg-secondary">
      <div className="flex items-center gap-1">
        {info?.is_admin ? (
          <>
            <Shield className="h-3 w-3 text-success" />
            <span>{t("status.admin")}</span>
          </>
        ) : (
          <>
            <ShieldOff className="h-3 w-3 text-warning" />
            <span>{t("status.non-admin")}</span>
          </>
        )}
      </div>
      <div className="h-3 w-px bg-border" />
      <div className="flex items-center gap-1">
        {!networkPaused ? (
          <>
            <Link2 className="h-3 w-3 text-success" />
            <span>{t("status.network-monitoring")}</span>
          </>
        ) : (
          <>
            <Link2Off className="h-3 w-3 text-fg-tertiary" />
            <span>{t("status.network-idle")}</span>
          </>
        )}
      </div>
      <div className="h-3 w-px bg-border" />
      <div className="flex items-center gap-1">
        {collecting ? (
          <>
            <Activity className="h-3 w-3 text-success" />
            <span>{t("status.sysmon-collecting")}</span>
          </>
        ) : sysmon?.installed ? (
          <>
            <Activity className="h-3 w-3 text-warning" />
            <span>{t("status.sysmon-installed")}</span>
          </>
        ) : (
          <>
            <Activity className="h-3 w-3 text-fg-tertiary" />
            <span>{t("status.sysmon-not-installed")}</span>
          </>
        )}
      </div>
      <div className="h-3 w-px bg-border" />
      <div className="flex items-center gap-1">
        {autorunsScanned ? (
          <>
            <Search className="h-3 w-3 text-success" />
            <span>{t("status.autoruns-scanned")}</span>
          </>
        ) : (
          <>
            <SearchX className="h-3 w-3 text-fg-tertiary" />
            <span>{t("status.autoruns-not-scanned")}</span>
          </>
        )}
      </div>
      <div className="flex-1" />
      <div className="flex items-center gap-1">
        <Clock className="h-3 w-3" />
        <span className="font-mono">
          {formatEpochMillis(now.getTime())}
        </span>
      </div>
    </footer>
  );
}
