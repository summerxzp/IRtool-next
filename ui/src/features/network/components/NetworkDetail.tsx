import { useTranslation } from "react-i18next";
import { X, RefreshCw } from "lucide-react";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { NetConn, CmdlineStatus } from "../types";
import * as api from "../api";

interface Props {
  conn: NetConn | null;
  onClose?: () => void;
}

function fmtTime(epoch: number) {
  if (!epoch) return "-";
  return new Date(epoch * 1000).toLocaleString("en-GB", { hour12: false });
}

function cmdlineStatusLabel(status: CmdlineStatus): string {
  switch (status) {
    case "unknown": return "Unknown";
    case "pending": return "Loading...";
    case "ready": return "Ready";
    case "denied": return "Access Denied";
    case "exited": return "Process Exited";
    case "failed": return "Query Failed";
  }
}

function cmdlineStatusVariant(status: CmdlineStatus): "default" | "success" | "warning" | "danger" | "info" {
  switch (status) {
    case "ready": return "success";
    case "pending": return "info";
    case "denied": return "warning";
    case "exited": return "default";
    case "failed": return "danger";
    default: return "default";
  }
}

export function NetworkDetail({ conn, onClose }: Props) {
  const { t } = useTranslation();
  const [refreshing, setRefreshing] = useState(false);

  const handleRefreshCmdline = async () => {
    if (!conn || refreshing) return;
    setRefreshing(true);
    try {
      await api.refreshCmdline(conn.pid);
    } catch (e) {
      console.error("Failed to refresh cmdline:", e);
    } finally {
      setRefreshing(false);
    }
  };

  if (!conn) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("network.detail.select-row")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4 space-y-4">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="info">{conn.proto.toUpperCase()}</Badge>
            <Badge variant="outline">{conn.family.toUpperCase()}</Badge>
            {conn.state && conn.state !== "NONE" && (
              <Badge>{conn.state}</Badge>
            )}
            {!conn.is_current && <Badge variant="warning">history</Badge>}
          </div>
          <div className="text-sm font-mono text-fg-primary">
            {conn.local.addr}:{conn.local.port} → {conn.remote.addr || "*"}:
            {conn.remote.port || "*"}
          </div>
        </div>
        {onClose && (
          <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>

      <Separator />

      <div>
        <div className="text-xs text-fg-tertiary mb-1">{t("network.detail.process")}</div>
        <div className="text-sm">
          <span className="text-fg-primary">{conn.process_name ?? "-"}</span>
          <span className="text-fg-tertiary ml-2 font-mono text-xs">PID {conn.pid}</span>
        </div>
        {conn.process_path && (
          <div className="text-xs font-mono text-fg-secondary mt-1 break-all">
            {conn.process_path}
          </div>
        )}
      </div>

      <Separator />

      <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
        <div>
          <div className="text-fg-tertiary">{t("network.detail.first-seen")}</div>
          <div className="font-mono text-fg-secondary">{fmtTime(conn.first_seen)}</div>
        </div>
        <div>
          <div className="text-fg-tertiary">{t("network.detail.last-seen")}</div>
          <div className="font-mono text-fg-secondary">{fmtTime(conn.last_seen)}</div>
        </div>
      </div>

      <Separator />

      <div>
        <div className="text-xs text-fg-tertiary mb-1">{t("network.detail.command-line")}</div>
        <div className="flex items-center gap-2 mb-1">
          <Badge variant={cmdlineStatusVariant(conn.cmdline_status)}>
            {cmdlineStatusLabel(conn.cmdline_status)}
          </Badge>
          <button
            className="text-fg-tertiary hover:text-fg-primary p-0.5 disabled:opacity-50"
            onClick={handleRefreshCmdline}
            disabled={refreshing}
            title={t("network.detail.refresh-cmdline")}
          >
            <RefreshCw className={`h-3.5 w-3.5 ${refreshing ? "animate-spin" : ""}`} />
          </button>
        </div>
        <div className="text-xs font-mono text-fg-secondary break-all">
          {conn.process_cmdline || t("network.detail.command-line-pending")}
        </div>
      </div>
    </div>
  );
}
