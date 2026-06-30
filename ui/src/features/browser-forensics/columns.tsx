import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import type { ExtensionInfo, DownloadInfo, HistoryEntry } from "./types";
import { formatTimestamp } from "./utils";

const RISK_VARIANT_MAP: Record<string, "danger" | "warning" | "info"> = {
  high_privilege_combo: "danger",
  broad_host_access: "warning",
  content_script_inject: "info",
  side_loaded: "warning",
  unknown_update_url: "warning",
  preferences_tampered: "danger",
  recently_installed: "info",
};

function RiskBadges({ flags }: { flags: string[] }) {
  const { t } = useTranslation();
  if (flags.length === 0) return null;
  return (
    <span className="flex gap-1 flex-wrap">
      {flags.map((f) => (
        <Badge key={f} variant={RISK_VARIANT_MAP[f] ?? "info"}>
          {t(`browser-forensics.risk.${f.replace(/_/g, "-")}`, f)}
        </Badge>
      ))}
    </span>
  );
}

export function useExtensionColumns(): ColumnDef<ExtensionInfo, unknown>[] {
  const { t } = useTranslation();
  return useMemo(
    () => [
      {
        id: "enabled",
        accessorFn: (r) => r.enabled,
        header: t("browser-forensics.enabled"),
        size: 60,
        cell: ({ row }) =>
          row.original.enabled ? (
            <span className="text-success text-xs">✓</span>
          ) : (
            <span className="text-fg-tertiary text-xs">✗</span>
          ),
      },
      {
        id: "name",
        accessorFn: (r) => r.name,
        header: t("browser-forensics.col.name"),
        size: 180,
      },
      {
        id: "version",
        accessorFn: (r) => r.version,
        header: t("browser-forensics.col.version"),
        size: 70,
      },
      {
        id: "id",
        accessorFn: (r) => r.id,
        header: "ID",
        size: 140,
        cell: ({ row }) => (
          <span className="font-mono text-xs text-fg-secondary">{row.original.id}</span>
        ),
      },
      {
        id: "risk_flags",
        accessorFn: (r) => r.risk_flags.length,
        header: t("browser-forensics.col.risk"),
        size: 200,
        cell: ({ row }) => <RiskBadges flags={row.original.risk_flags} />,
      },
      {
        id: "permissions_count",
        accessorFn: (r) => r.permissions.length,
        header: t("browser-forensics.col.permissions"),
        size: 70,
        cell: ({ row }) => <span>{row.original.permissions.length}</span>,
      },
      {
        id: "install_source",
        accessorFn: (r) => r.install_source ?? "",
        header: t("browser-forensics.col.install-source"),
        size: 120,
      },
      {
        id: "install_time",
        accessorFn: (r) => r.install_time ?? "",
        header: t("browser-forensics.col.install-time"),
        size: 150,
        cell: ({ row }) => <span>{formatTimestamp(row.original.install_time)}</span>,
      },
    ],
    [t],
  );
}

export function useDownloadColumns(): ColumnDef<DownloadInfo, unknown>[] {
  const { t } = useTranslation();
  return useMemo(
    () => [
      {
        id: "filename",
        accessorFn: (r) => r.filename,
        header: t("browser-forensics.col.filename"),
        size: 200,
      },
      {
        id: "download_url",
        accessorFn: (r) => r.download_url,
        header: "URL",
        size: 300,
        cell: ({ row }) => (
          <span className="font-mono text-xs text-fg-secondary truncate">{row.original.download_url}</span>
        ),
      },
      {
        id: "referrer",
        accessorFn: (r) => r.referrer ?? "",
        header: t("browser-forensics.col.referrer"),
        size: 300,
        cell: ({ row }) => {
          const val = row.original.referrer;
          if (!val) {
            return <span className="text-fg-tertiary text-xs">—</span>;
          }
          return (
            <span className="font-mono text-xs text-fg-secondary truncate">{val}</span>
          );
        },
      },
      {
        id: "danger_type",
        accessorFn: (r) => r.danger_type,
        header: t("browser-forensics.col.danger-type"),
        size: 140,
        cell: ({ row }) => {
          const dt = row.original.danger_type;
          if (dt === "NOT_DANGEROUS") return null;
          return <Badge variant="warning">{dt.replace(/_/g, " ")}</Badge>;
        },
      },
      {
        id: "start_time",
        accessorFn: (r) => r.start_time ?? "",
        header: t("browser-forensics.col.start-time"),
        size: 150,
        cell: ({ row }) => <span>{formatTimestamp(row.original.start_time)}</span>,
      },
      {
        id: "total_bytes",
        accessorFn: (r) => r.total_bytes,
        header: t("browser-forensics.col.size"),
        size: 80,
        cell: ({ row }) => {
          const b = row.original.total_bytes;
          return b != null ? formatSize(b) : "";
        },
      },
      {
        id: "opened",
        accessorFn: (r) => r.opened,
        header: t("browser-forensics.col.opened"),
        size: 60,
        cell: ({ row }) =>
          row.original.opened ? (
            <span className="text-success text-xs">✓</span>
          ) : null,
      },
    ],
    [t],
  );
}

export function useHistoryColumns(): ColumnDef<HistoryEntry, unknown>[] {
  const { t } = useTranslation();
  return useMemo(
    () => [
      {
        id: "visit_time",
        accessorFn: (r) => r.visit_time,
        header: t("browser-forensics.col.visit-time"),
        size: 150,
        cell: ({ row }) => <span>{formatTimestamp(row.original.visit_time)}</span>,
      },
      {
        id: "title",
        accessorFn: (r) => r.title,
        header: t("browser-forensics.col.title"),
        size: 250,
      },
      {
        id: "url",
        accessorFn: (r) => r.url,
        header: "URL",
        size: 350,
        cell: ({ row }) => (
          <span className="font-mono text-xs text-fg-secondary truncate">{row.original.url}</span>
        ),
      },
      {
        id: "visit_count",
        accessorFn: (r) => r.visit_count,
        header: t("browser-forensics.col.visit-count"),
        size: 80,
      },
    ],
    [t],
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
