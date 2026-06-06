import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useAlertStore } from "@/stores/alert-store";
import { ChevronDown, ChevronRight, Trash2 } from "lucide-react";

const EVENT_TYPE_LABELS: Record<string, string> = {
  dns: "DNS查询",
  dns_client: "DNS-Client",
  network_connect: "网络连接",
  tls_sni: "TLS SNI",
  dns_pcap: "DNS抓包",
  create_remote_thread: "远程线程",
  file_create: "文件创建",
};

interface Props {
  onClose: () => void;
}

type DetailCell = { label: string; value: string };
type DetailRow =
  | { type: "two-col"; left: DetailCell; right: DetailCell }
  | { type: "full"; cell: DetailCell };

function parseRawJson(raw: string, alertTimestamp?: number): DetailRow[] {
  try {
    const v = JSON.parse(raw);
    const rows: DetailRow[] = [];

    const isPcap = "event_kind" in v && ("domain" in v || "src_ip" in v);
    const isDns = !isPcap && "query_name" in v;
    const isNetwork = !isPcap && !isDns && ("source_ip" in v || "protocol" in v);

    if (isNetwork) {
      // Sysmon network_connect
      if (v.protocol || v.user)
        rows.push({
          type: "two-col",
          left: { label: "协议", value: v.protocol ?? "-" },
          right: { label: "用户", value: v.user ?? "-" },
        });
      if (v.source_ip || v.destination_ip)
        rows.push({
          type: "two-col",
          left: { label: "源地址", value: v.source_ip ? `${v.source_ip}:${v.source_port ?? ""}` : "-" },
          right: { label: "目标地址", value: v.destination_ip ? `${v.destination_ip}:${v.destination_port ?? ""}` : "-" },
        });
      if (v.process_name || v.process_id)
        rows.push({
          type: "two-col",
          left: { label: "进程", value: v.process_name ? `${v.process_name} (${v.process_id ?? "-"})` : "-" },
          right: { label: "发起", value: v.initiated !== undefined ? (v.initiated ? "是" : "否") : "-" },
        });
      if (alertTimestamp)
        rows.push({ type: "full", cell: { label: "时间", value: new Date(alertTimestamp).toLocaleString() } });
      if (v.process_path)
        rows.push({ type: "full", cell: { label: "路径", value: v.process_path } });
      if (v.process_chain)
        rows.push({ type: "full", cell: { label: "进程链", value: v.process_chain } });
    } else if (isDns) {
      // Sysmon DNS
      if (v.query_name)
        rows.push({ type: "full", cell: { label: "查询域名", value: v.query_name } });
      if (v.query_results)
        rows.push({ type: "full", cell: { label: "解析结果", value: v.query_results } });
      if (v.query_status !== undefined)
        rows.push({ type: "full", cell: { label: "查询状态", value: String(v.query_status) } });
      if (v.process_path)
        rows.push({ type: "full", cell: { label: "路径", value: v.process_path } });
      if (v.process_name || v.process_id)
        rows.push({
          type: "two-col",
          left: { label: "进程", value: v.process_name ? `${v.process_name} (${v.process_id ?? "-"})` : "-" },
          right: { label: "用户", value: v.user ?? "-" },
        });
    } else if (isPcap) {
      // Pcap (tls_sni / dns_query)
      const kindLabel = v.event_kind === "tls_sni" ? "TLS SNI" : "DNS查询";
      if (v.event_kind)
        rows.push({ type: "full", cell: { label: "类型", value: kindLabel } });
      if (v.domain)
        rows.push({ type: "full", cell: { label: "域名", value: v.domain } });
      if (v.src_ip || v.dst_ip)
        rows.push({
          type: "two-col",
          left: { label: "源地址", value: v.src_ip ? `${v.src_ip}:${v.src_port ?? ""}` : "-" },
          right: { label: "目标地址", value: v.dst_ip ? `${v.dst_ip}:${v.dst_port ?? ""}` : "-" },
        });
      if (v.query_type)
        rows.push({ type: "full", cell: { label: "查询类型", value: v.query_type } });
    } else {
      // Fallback: show all key-value pairs
      for (const [key, val] of Object.entries(v)) {
        if (val !== undefined && val !== null && val !== "") {
          rows.push({ type: "full", cell: { label: key, value: String(val) } });
        }
      }
    }

    return rows;
  } catch {
    return [];
  }
}

export function AlertPanel({ onClose }: Props) {
  const { t } = useTranslation();
  const { alerts, readIds, markAllRead, markRead, clearAlerts } = useAlertStore();
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on click outside
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  const formatTime = (ts: number) => {
    return new Date(ts).toLocaleString();
  };

  const handleClear = () => {
    if (confirmClear) {
      clearAlerts();
      setConfirmClear(false);
    } else {
      setConfirmClear(true);
      setTimeout(() => setConfirmClear(false), 3000);
    }
  };

  return (
    <div ref={panelRef} className="absolute right-12 top-9 w-96 max-h-[70vh] bg-bg-elev-1 border border-border rounded-md shadow-lg z-50 flex flex-col">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <span className="text-xs font-medium">{t("alert.title")}</span>
        <div className="flex items-center gap-2">
          <button
            className="text-[10px] text-accent hover:underline"
            onClick={handleClear}
          >
            {confirmClear ? t("alert.confirm-clear") : (
              <Trash2 className="h-3 w-3 text-fg-tertiary hover:text-red-500" />
            )}
          </button>
          <button
            className="text-[10px] text-accent hover:underline"
            onClick={markAllRead}
          >
            {t("alert.mark-all-read")}
          </button>
        </div>
      </div>

      <div className="overflow-y-auto flex-1">
        {alerts.length === 0 && (
          <p className="text-xs text-fg-tertiary py-6 text-center">{t("alert.no-alerts")}</p>
        )}
        {alerts.map((alert) => {
          const isExpanded = expandedId === alert.id;
          const isUnread = !readIds.has(alert.id);
          const details = isExpanded ? parseRawJson(alert.raw_json, alert.timestamp) : [];
          const hasDetails = parseRawJson(alert.raw_json, alert.timestamp).length > 0;
          return (
            <div
              key={`${alert.id}-${alert.timestamp}`}
              className="px-3 py-2 border-b border-border/50 hover:bg-bg-elev-2 cursor-pointer"
              onClick={() => {
                setExpandedId(isExpanded ? null : alert.id);
                if (isUnread) markRead(alert.id);
              }}
            >
              <div className="flex items-start gap-2">
                {isUnread && <span className="text-red-500 text-xs mt-0.5">●</span>}
                {!isUnread && <span className="text-xs mt-0.5 w-[7px]" />}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-medium text-fg-primary">{alert.rule_name}</span>
                    <span className="text-[10px] text-fg-tertiary shrink-0 ml-2">{formatTime(alert.timestamp)}</span>
                  </div>
                  <p className="text-[11px] text-fg-secondary mt-0.5 truncate">
                    {alert.key_field}
                  </p>
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-[10px] text-fg-tertiary">{EVENT_TYPE_LABELS[alert.event_type] || alert.event_type}</span>
                    {alert.process_name && (
                      <span className="text-[10px] text-fg-tertiary">| {alert.process_name}</span>
                    )}
                  </div>

                  {isExpanded && details.length > 0 && (
                    <div className="mt-1.5 pl-2 border-l-2 border-accent/30 space-y-0.5">
                      {details.map((row, i) =>
                        row.type === "two-col" ? (
                          <div key={i} className="grid grid-cols-2 gap-x-3 text-[10px]">
                            <div>
                              <span className="text-fg-tertiary">{row.left.label}: </span>
                              <span className="text-fg-secondary break-all">{row.left.value}</span>
                            </div>
                            <div>
                              <span className="text-fg-tertiary">{row.right.label}: </span>
                              <span className="text-fg-secondary break-all">{row.right.value}</span>
                            </div>
                          </div>
                        ) : (
                          <div key={i} className="text-[10px]">
                            <span className="text-fg-tertiary">{row.cell.label}: </span>
                            <span className="text-fg-secondary break-all">{row.cell.value}</span>
                          </div>
                        )
                      )}
                    </div>
                  )}
                </div>
                {hasDetails ? (
                  isExpanded ? (
                    <ChevronDown className="h-3 w-3 text-fg-tertiary mt-1 shrink-0" />
                  ) : (
                    <ChevronRight className="h-3 w-3 text-fg-tertiary mt-1 shrink-0" />
                  )
                ) : null}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
