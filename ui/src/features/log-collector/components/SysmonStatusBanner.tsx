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
      <div className="flex items-center gap-2 px-3 py-2 bg-danger-bg border-b border-danger-border text-xs">
        <ShieldOff className="h-4 w-4 text-danger shrink-0" />
        <span className="text-danger font-medium">{t("log-collector.banner.not-installed")}</span>
        <span className="text-fg-secondary flex-1">{t("log-collector.banner.not-installed-desc")}</span>
        <Button variant="secondary" size="sm" className="h-6 text-xs border-danger-border text-danger hover:bg-danger-bg" onClick={onInstall}>
          {t("log-collector.banner.install")}
        </Button>
      </div>
    );
  }

  if (!status.running) {
    return (
      <div className="flex items-center gap-2 px-3 py-2 bg-warning-bg border-b border-warning-border text-xs">
        <ShieldAlert className="h-4 w-4 text-warning shrink-0" />
        <span className="text-warning font-medium">{t("log-collector.banner.not-running")}</span>
        <span className="text-fg-secondary flex-1">{t("log-collector.banner.not-running-desc")}</span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-success-bg border-b border-success-border text-xs">
      <ShieldCheck className="h-4 w-4 text-success shrink-0" />
      <span className="text-success font-medium">{t("log-collector.banner.running")}</span>
      <span className="text-fg-secondary">
        {status.service_name && `· ${status.service_name}`}
        {status.started_by_irtool && ` · ${t("log-collector.banner.managed-by-irtool")}`}
      </span>
    </div>
  );
}
