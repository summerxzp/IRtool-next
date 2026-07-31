import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { formatEpochMillis } from "@/lib/utils";
import * as api from "../api";

type Status = api.ExtensionConnectionStatus | null;

/// 相对时间格式化（紧凑）：
/// - < 60s → 秒级（i18n）
/// - < 60min → 分钟级（i18n）
/// - 否则 → UTC+8 绝对时间
function formatRelative(ms: number, t: TFunction): string {
  const diff = Date.now() - ms;
  if (diff < 60_000) {
    const count = Math.max(0, Math.floor(diff / 1000));
    return t("browser-forensics.connection.last-heartbeat-seconds", { count });
  }
  if (diff < 3_600_000) {
    const count = Math.floor(diff / 60_000);
    return t("browser-forensics.connection.last-heartbeat-minutes", { count });
  }
  return formatEpochMillis(ms);
}

export function ExtensionConnectionBadge() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<Status>(null);

  useEffect(() => {
    let cancelled = false;
    const poll = () => {
      api
        .getExtensionStatus()
        .then((s) => {
          if (!cancelled) setStatus(s);
        })
        .catch(() => {
          // 轮询失败时保持上次状态，不打扰用户
        });
    };
    poll();
    const id = setInterval(poll, 5000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  const connected = status?.connected ?? false;
  const statusLabel = connected
    ? t("browser-forensics.connection.connected", { defaultValue: "扩展已连接" })
    : t("browser-forensics.connection.disconnected", { defaultValue: "扩展未连接" });

  return (
    <span
      className="inline-flex items-center gap-1.5 h-8 px-2 text-xs select-none"
      role="status"
      aria-label={statusLabel}
    >
      <span
        className={`h-2 w-2 rounded-full shrink-0 ${connected ? "bg-success" : "bg-muted-foreground/40"}`}
      />
      <span className={connected ? "text-success" : "text-muted-foreground"}>
        {statusLabel}
      </span>
      {connected && status && status.last_heartbeat_ms > 0 && (
        <span className="text-muted-foreground/70">
          ({formatRelative(status.last_heartbeat_ms, t)})
        </span>
      )}
    </span>
  );
}
