import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { NetConn } from "../types";

interface Props {
  conn: NetConn | null;
}

function fmtTime(epoch: number) {
  if (!epoch) return "-";
  return new Date(epoch * 1000).toLocaleString("en-GB", { hour12: false });
}

export function NetworkDetail({ conn }: Props) {
  const { t } = useTranslation();

  if (!conn) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("network.detail.select-row")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-4 space-y-4">
      <div>
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

      <div>
        <div className="text-xs text-fg-tertiary mb-1">{t("network.detail.command-line")}</div>
        <div className="text-xs font-mono text-fg-tertiary italic">
          {t("network.detail.command-line-pending")}
        </div>
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
    </div>
  );
}
