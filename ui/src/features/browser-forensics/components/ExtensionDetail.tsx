import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import type { ExtensionInfo } from "../types";
import { formatTimestamp } from "../utils";

interface Props {
  item: ExtensionInfo | null;
  onClose?: () => void;
}

const RISK_VARIANT_MAP: Record<string, "danger" | "warning" | "info"> = {
  high_privilege_combo: "danger",
  broad_host_access: "warning",
  content_script_inject: "info",
  side_loaded: "warning",
  unknown_update_url: "warning",
  preferences_tampered: "danger",
  recently_installed: "info",
};

export function ExtensionDetail({ item, onClose }: Props) {
  const { t } = useTranslation();

  if (!item) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("browser-forensics.detail.select-extension")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4 space-y-3">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-sm font-medium text-fg-primary">{item.name}</div>
          <div className="flex items-center gap-1.5 mt-1 text-xs text-fg-tertiary flex-wrap">
            <span>v{item.version}</span>
            <span>·</span>
            <span>{item.id}</span>
            {!item.enabled && (
              <>
                <span>·</span>
                <span className="text-fg-tertiary">{t("browser-forensics.disabled")}</span>
              </>
            )}
          </div>
        </div>
        {onClose && (
          <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>

      <Separator />

      {item.risk_flags.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {item.risk_flags.map((f) => (
            <Badge key={f} variant={RISK_VARIANT_MAP[f] ?? "info"}>
              {t(`browser-forensics.risk.${f.replace(/_/g, "-")}`, f)}
            </Badge>
          ))}
        </div>
      )}

      {item.ioc_matches.length > 0 && (
        <div className="space-y-1">
          <div className="text-xs text-fg-tertiary">{t("browser-forensics.detail.ioc-matches")}</div>
          {item.ioc_matches.map((m, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <Badge variant="danger">{m.severity}</Badge>
              <span className="text-fg-tertiary">{m.ioc_type}:</span>
              <span className="font-mono break-all">{m.value}</span>
            </div>
          ))}
        </div>
      )}

      <div className="space-y-2 text-xs">
        <DetailRow label={t("browser-forensics.detail.description")} value={item.description} />
        <DetailRow label={t("browser-forensics.detail.install-time")} value={formatTimestamp(item.install_time)} />
        <DetailRow label={t("browser-forensics.detail.install-source")} value={item.install_source} />
        <DetailRow label={t("browser-forensics.detail.update-url")} value={item.update_url} />
        <DetailRow label={t("browser-forensics.detail.path")} value={item.path} mono />
        <DetailRow
          label={t("browser-forensics.detail.was-installed-by-default")}
          value={item.was_installed_by_default != null ? (item.was_installed_by_default ? "Yes" : "No") : null}
        />
        <DetailRow
          label={t("browser-forensics.detail.has-content-scripts")}
          value={item.has_content_scripts ? "Yes" : "No"}
        />
        <DetailRow
          label={t("browser-forensics.detail.has-background")}
          value={item.has_background ? "Yes" : "No"}
        />
        <DetailRow
          label={t("browser-forensics.detail.preferences-tampered")}
          value={item.preferences_tampered ? "Yes" : "No"}
        />
      </div>

      {item.permissions.length > 0 && (
        <>
          <Separator />
          <div className="text-xs">
            <div className="text-fg-tertiary mb-1">{t("browser-forensics.detail.permissions")}</div>
            <div className="flex gap-1 flex-wrap">
              {item.permissions.map((p) => (
                <Badge key={p} variant="outline">{p}</Badge>
              ))}
            </div>
          </div>
        </>
      )}

      {item.host_permissions.length > 0 && (
        <>
          <Separator />
          <div className="text-xs">
            <div className="text-fg-tertiary mb-1">{t("browser-forensics.detail.host-permissions")}</div>
            <div className="flex gap-1 flex-wrap">
              {item.host_permissions.map((p) => (
                <Badge key={p} variant="outline">{p}</Badge>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function DetailRow({ label, value, mono = false }: { label: string; value?: string | null; mono?: boolean }) {
  if (!value) return null;
  return (
    <div>
      <div className="text-fg-tertiary">{label}</div>
      <div className={mono ? "font-mono break-all" : "break-all"}>{value}</div>
    </div>
  );
}
