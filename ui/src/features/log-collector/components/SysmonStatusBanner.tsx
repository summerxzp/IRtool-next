import { useTranslation } from "react-i18next";
import { ShieldCheck, ShieldAlert, ShieldOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { SysmonStatus } from "../types";

interface Props {
  status: SysmonStatus | null;
  onInstall: () => void;
}

export function SysmonStatusBanner({ status, onInstall }: Props) {
  const { t } = useTranslation();

  if (!status) return null;

  if (!status.installed) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 bg-red-500/10 border-b border-red-500/20 text-xs">
        <ShieldOff className="h-4 w-4 text-red-500 shrink-0" />
        <span className="text-red-500 font-medium">{t("log-collector.banner.not-installed")}</span>
        <span className="text-fg-secondary flex-1">{t("log-collector.banner.not-installed-desc")}</span>
        <Button variant="secondary" size="sm" className="h-6 text-xs border-red-500/30 text-red-500 hover:bg-red-500/10" onClick={onInstall}>
          {t("log-collector.banner.install")}
        </Button>
      </div>
    );
  }

  if (!status.running) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 bg-yellow-500/10 border-b border-yellow-500/20 text-xs">
        <ShieldAlert className="h-4 w-4 text-yellow-500 shrink-0" />
        <span className="text-yellow-500 font-medium">{t("log-collector.banner.not-running")}</span>
        <span className="text-fg-secondary flex-1">{t("log-collector.banner.not-running-desc")}</span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-green-500/10 border-b border-green-500/20 text-xs">
      <ShieldCheck className="h-4 w-4 text-green-500 shrink-0" />
      <span className="text-green-500 font-medium">{t("log-collector.banner.running")}</span>
      <span className="text-fg-secondary">
        {status.service_name && `· ${status.service_name}`}
        {status.started_by_irtool && ` · ${t("log-collector.banner.managed-by-irtool")}`}
      </span>
    </div>
  );
}
