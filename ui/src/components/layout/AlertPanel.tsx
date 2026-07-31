import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useAlertStore, alertKey } from "@/stores/alert-store";
import { formatEpochMillis } from "@/lib/utils";
import { ChevronDown, ChevronRight, Trash2, ChevronsUpDown } from "lucide-react";

const EVENT_TYPE_LABELS: Record<string, string> = {
  dns: "DNS查询",
  dns_client: "DNS-Client",
  network_connect: "网络连接",
  network_monitor: "网络监控",
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

function parseRawJson(raw: string): DetailRow[] {
  try {
    const v = JSON.parse(raw);
    const rows: DetailRow[] = [];

    const isPcap = "event_kind" in v && ("domain" in v || "src_ip" in v);
    const isDns = !isPcap && "query_name" in v;
    const isNetwork = !isPcap && !isDns && ("source_ip" in v || "protocol" in v);
    const isNetMonitor = !isPcap && !isDns && !isNetwork && ("local" in v || "remote" in v);

    if (isNetMonitor) {
      // Network monitor (irtool-net-monitor) event
      const local = v.local as { addr?: string; port?: number };
      const remote = v.remote as { addr?: string; port?: number };
      if (local?.addr)
        rows.push({ type: "full", cell: { label: "源地址", value: `${local.addr}${local.port ? `:${local.port}` : ""}` } });
      if (remote?.addr)
        rows.push({ type: "full", cell: { label: "目标地址", value: `${remote.addr}${remote.port ? `:${remote.port}` : ""}` } });
      if (v.proto)
        rows.push({ type: "two-col", left: { label: "协议", value: String(v.proto).toUpperCase() }, right: { label: "状态", value: String(v.state ?? "-") } });
      if (v.process_name || v.pid)
        rows.push({ type: "two-col", left: { label: "进程", value: `${v.process_name || "-"}${v.pid ? ` (${v.pid})` : ""}` }, right: { label: "PID", value: String(v.pid ?? "-") } });
      if (v.process_path)
        rows.push({ type: "full", cell: { label: "路径", value: String(v.process_path) } });
    } else if (isNetwork) {
      // Sysmon network_connect
      if (v.source_ip)
        rows.push({
          type: "two-col",
          left: { label: "源地址", value: `${v.source_ip}${v.source_port ? `:${v.source_port}` : ""}` },
          right: { label: "用户", value: v.user ?? "-" },
        });
      if (v.destination_ip)
        rows.push({
          type: "full",
          cell: { label: "目标地址", value: `${v.destination_ip}${v.destination_port ? `:${v.destination_port}` : ""}` },
        });
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
      if (v.query_status !== undefined && v.query_status !== 0)
        rows.push({ type: "full", cell: { label: "查询状态", value: String(v.query_status) } });
      if (v.process_path)
        rows.push({ type: "full", cell: { label: "路径", value: v.process_path } });
      if (v.process_name || v.process_id || v.source_ip)
        rows.push({
          type: "two-col",
          left: { label: "源IP", value: v.source_ip ? `${v.source_ip}${v.source_port ? `:${v.source_port}` : ""}` : (v.process_name ? `${v.process_name} (${v.process_id ?? "-"})` : "-") },
          right: { label: "用户", value: v.user ?? "-" },
        });
      if (v.process_chain)
        rows.push({ type: "full", cell: { label: "进程链", value: v.process_chain } });
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
      if (v.process_chain)
        rows.push({ type: "full", cell: { label: "进程链", value: v.process_chain } });
    } else {
      // Fallback: show all key-value pairs
      for (const [key, val] of Object.entries(v)) {
        if (val !== undefined && val !== null && val !== "" && typeof val !== "object") {
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
  const highlightedAlertKey = useAlertStore((s) => s.highlightedAlertKey);
  const setHighlightedAlertKey = useAlertStore((s) => s.setHighlightedAlertKey);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [allExpanded, setAllExpanded] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on click outside (excluding the alert toggle button)
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest("[data-alert-toggle]")) return;
      if (panelRef.current && !panelRef.current.contains(target)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  // Auto-expand and scroll to highlighted alert when notification is clicked
  useEffect(() => {
    if (highlightedAlertKey != null) {
      const key = highlightedAlertKey;
      setExpandedIds((prev) => new Set(prev).add(key));
      setHighlightedAlertKey(null);
      setTimeout(() => {
        const el = document.querySelector(`[data-alert-key="${key}"]`);
        el?.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 100);
    }
  }, [highlightedAlertKey, setHighlightedAlertKey]);

  const formatTime = (ts: number) => {
    // If timestamp is in seconds (less than year 3000 in ms), convert to ms
    const ms = ts > 1e12 ? ts : ts * 1000;
    return formatEpochMillis(ms);
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
            onClick={() => {
              if (allExpanded) {
                setAllExpanded(false);
                setExpandedIds(new Set());
              } else {
                setAllExpanded(true);
                setExpandedIds(new Set(alerts.map((a) => alertKey(a))));
              }
            }}
            title={allExpanded ? t("alert.collapse-all") : t("alert.expand-all")}
          >
            <ChevronsUpDown className="h-3 w-3 text-fg-tertiary hover:text-fg-secondary" />
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
          const key = alertKey(alert);
          const isExpanded = expandedIds.has(key);
          const isUnread = !readIds.has(key);
          const details = isExpanded ? parseRawJson(alert.raw_json) : [];
          const hasDetails = parseRawJson(alert.raw_json).length > 0;
          return (
            <div
              key={key}
              data-alert-key={key}
              className="px-3 py-2 border-b border-border/50 hover:bg-bg-elev-2 cursor-pointer"
              onClick={() => {
                setExpandedIds((prev) => {
                  const next = new Set(prev);
                  if (next.has(key)) next.delete(key);
                  else next.add(key);
                  return next;
                });
                if (isUnread) markRead(key);
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
                  <div className="flex items-center gap-1 mt-0.5 text-[10px] text-fg-tertiary">
                    <span>{EVENT_TYPE_LABELS[alert.event_type] || alert.event_type}</span>
                    {(() => {
                      try {
                        const raw = JSON.parse(alert.raw_json);
                        if (raw.protocol) return <>{` | ${raw.protocol.toUpperCase()}`}</>;
                        if (raw.proto) return <>{` | ${raw.proto.toUpperCase()}`}</>;
                        if (raw.event_kind === "tls_sni") return <>{" | TCP"}</>;
                        if (raw.event_kind === "dns_query") return <>{" | UDP"}</>;
                      } catch {}
                      return null;
                    })()}
                    {alert.process_name && (
                      <>{` | ${alert.process_name}${(() => {
                        try {
                          const raw = JSON.parse(alert.raw_json);
                          const pid = raw.process_id || raw.pid;
                          return pid ? ` (${pid})` : "";
                        } catch { return ""; }
                      })()}`}</>
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
