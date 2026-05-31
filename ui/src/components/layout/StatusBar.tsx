import { useEffect, useState } from "react";
import { commands, type AppInfo } from "@/lib/bindings";
import { useTranslation } from "react-i18next";
import { Shield, ShieldOff, Clock } from "lucide-react";

export function StatusBar() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    commands.cmdAppInfo().then(setInfo).catch(() => null);
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
      <div>{t("status.sysmon-not-installed")}</div>
      <div className="flex-1" />
      <div className="flex items-center gap-1">
        <Clock className="h-3 w-3" />
        <span className="font-mono">
          {now.toLocaleTimeString("en-GB", { hour12: false })}
        </span>
      </div>
    </footer>
  );
}
