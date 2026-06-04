import { useTranslation } from "react-i18next";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { SignatureBadge } from "./SignatureBadge";
import type { AutorunItem } from "../types";

interface Props {
  item: AutorunItem | null;
  onDelete: (item: AutorunItem) => void;
  onJumpToRegistry: (item: AutorunItem) => void;
  onSearchInWorkspace: (item: AutorunItem) => void;
}

export function AutorunsDetail({ item, onDelete, onJumpToRegistry, onSearchInWorkspace }: Props) {
  const { t } = useTranslation();

  if (!item) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("autoruns.detail.select-row")}
      </div>
    );
  }

  const canJumpToRegistry = item.location.includes("HKLM") || item.location.includes("HKCU");
  const canJumpToTask = item.category === "Scheduled Tasks" || item.category === "Tasks";

  return (
    <div className="h-full overflow-auto p-4 space-y-4">
      <div>
        <div className="flex items-center gap-2 mb-2">
          <SignatureBadge status={item.signature} />
          {!item.enabled && <span className="text-xs text-fg-tertiary">已禁用</span>}
        </div>
        <div className="text-sm font-medium text-fg-primary">{item.entry}</div>
        <div className="text-xs text-fg-tertiary">{item.category}</div>
      </div>

      <Separator />

      <div className="space-y-2 text-xs">
        <DetailRow label={t("autoruns.detail.image-path")} value={item.image_path} mono />
        <DetailRow label={t("autoruns.detail.launch-string")} value={item.launch_string} mono />
        <DetailRow label={t("autoruns.detail.location")} value={item.location} mono />
        <DetailRow label={t("autoruns.detail.publisher")} value={item.publisher} />
        <DetailRow label={t("autoruns.detail.description")} value={item.description} />
        <DetailRow label={t("autoruns.detail.timestamp")} value={item.timestamp} />
        {item.file_version && <DetailRow label={t("autoruns.detail.version")} value={item.file_version} />}
        {item.service_name && <DetailRow label={t("autoruns.detail.service-name")} value={item.service_name} />}
        {!item.file_exists && <div className="text-danger font-medium">{t("autoruns.detail.file-not-found")}</div>}
        {item.file_size != null && <DetailRow label={t("autoruns.detail.file-size")} value={formatSize(item.file_size)} />}
        {item.sha256 && <DetailRow label="SHA-256" value={item.sha256} mono />}
      </div>

      <Separator />

      <div className="flex flex-wrap gap-2">
        <Button variant="destructive" size="sm" onClick={() => onDelete(item)}>
          {t("autoruns.detail.delete")}
        </Button>
        {canJumpToRegistry && (
          <Button variant="secondary" size="sm" onClick={() => onJumpToRegistry(item)}>
            {t("autoruns.detail.jump-registry")}
          </Button>
        )}
        {canJumpToTask && (
          <Button variant="secondary" size="sm" onClick={() => onJumpToRegistry(item)}>
            {t("autoruns.detail.jump-task")}
          </Button>
        )}
        <Button variant="secondary" size="sm" disabled onClick={() => onSearchInWorkspace(item)}>
          {t("autoruns.detail.search-workspace")}
        </Button>
      </div>
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

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
