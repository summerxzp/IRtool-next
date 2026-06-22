import { useState, useCallback, useSyncExternalStore, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "@tanstack/react-router";
import { X, AlertTriangle } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { useProcessChain, useNetworkByPid, useAutorunsByPath } from "../hooks";
import { iconCache, subscribePath } from "../columns";
import type { ProcessEntry, ProcessNode } from "../types";
import type { NetConn, AutorunItem, SysmonEvent } from "@/lib/bindings";
import { useLogCollectorStore } from "@/features/log-collector/store";
import { EVENT_TYPE_LABELS, EVENT_TYPE_COLORS } from "@/features/log-collector/types";
import type { ExtendedSysmonEventType } from "@/features/log-collector/types";

type TabId = "chain" | "network" | "sysmon" | "autoruns";

interface Props {
  entry: ProcessEntry | null;
  snapshotTime: string | null;
  onClose?: () => void;
}

function ChainNodeIcon({ imagePath }: { imagePath: string | null }) {
  const subscribe = useCallback(
    (listener: () => void) => {
      if (!imagePath) return () => {};
      return subscribePath(imagePath, listener);
    },
    [imagePath]
  );

  const iconSrc = useSyncExternalStore(
    subscribe,
    () => {
      if (!imagePath) return null;
      const cached = iconCache.get(imagePath);
      return cached && cached !== "" ? cached : null;
    },
    () => null
  );

  if (iconSrc) {
    return <img src={iconSrc} alt="" className="w-4 h-4 shrink-0 object-contain" />;
  }
  return <span className="w-4 h-4 shrink-0 inline-block rounded-sm bg-bg-elev-2" />;
}

function ChainNodeDetail({ node }: { node: ProcessNode }) {
  const { t } = useTranslation();
  return (
    <div className="ml-2 pl-2 border-l-2 border-border space-y-1 py-1">
      {node.exe && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.path")}: </span>
          <span className="font-mono break-all">{node.exe}</span>
        </div>
      )}
      {node.cmdline && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.cmdline")}: </span>
          <span className="font-mono break-all">{node.cmdline}</span>
        </div>
      )}
      {node.create_time && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.create-time")}: </span>
          <span>{node.create_time}</span>
        </div>
      )}
      {node.is_suspicious && node.suspicious_reason && (
        <div className="text-xs text-warning flex items-center gap-0.5">
          <AlertTriangle className="h-3 w-3" />
          {node.suspicious_reason}
        </div>
      )}
    </div>
  );
}

function formatEndpoint(addr: string, port: number) {
  return `${addr}:${port}`;
}

function formatTimeOnly(ts: number) {
  const d = new Date(ts * 1000);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

function NetworkTab({ entry }: { entry: ProcessEntry }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const networkQuery = useNetworkByPid(entry.pid);
  const data = networkQuery.data;
  const isLoading = networkQuery.isLoading;

  return (
    <div className="space-y-2">
      {isLoading ? (
        <div className="text-xs text-fg-tertiary">{t("common.loading")}</div>
      ) : data && data.length > 0 ? (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-fg-tertiary select-none">
                <th className="text-left font-normal pr-3 pb-1">{t("process.network.proto")}</th>
                <th className="text-left font-normal pr-3 pb-1">{t("process.network.local")}</th>
                <th className="text-left font-normal pr-3 pb-1">{t("process.network.remote")}</th>
                <th className="text-left font-normal pr-3 pb-1">{t("process.network.state")}</th>
                <th className="text-left font-normal pb-1">{t("process.network.last-seen")}</th>
              </tr>
            </thead>
            <tbody>
              {data.map((conn: NetConn, i: number) => (
                <tr key={i} className="border-t border-border/50">
                  <td className="pr-3 py-0.5 font-mono">{conn.proto.toUpperCase()}</td>
                  <td className="pr-3 py-0.5 font-mono">{formatEndpoint(conn.local.addr, conn.local.port)}</td>
                  <td className="pr-3 py-0.5 font-mono">{formatEndpoint(conn.remote.addr, conn.remote.port)}</td>
                  <td className="pr-3 py-0.5">{conn.state}</td>
                  <td className="py-0.5 font-mono">{formatTimeOnly(conn.last_seen)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="text-xs text-fg-tertiary">{t("process.association.no-network")}</div>
      )}
      <button
        className="text-xs text-accent hover:underline select-none"
        onClick={() => navigate({ to: "/network", search: { pid: entry.pid } })}
      >
        {t("process.association.view-network")} →
      </button>
    </div>
  );
}

function AutorunsTab({ entry }: { entry: ProcessEntry }) {
  const { t } = useTranslation();
  const navigate = useNavigate();

  if (!entry.exe) {
    return (
      <div className="text-xs text-fg-tertiary">{t("process.association.no-exe-path")}</div>
    );
  }

  const autorunsQuery = useAutorunsByPath(entry.exe);
  const data = autorunsQuery.data;
  const isLoading = autorunsQuery.isLoading;

  return (
    <div className="space-y-2">
      {isLoading ? (
        <div className="text-xs text-fg-tertiary">{t("common.loading")}</div>
      ) : data && data.length > 0 ? (
        <div className="space-y-1">
          {data.map((item: AutorunItem) => (
            <div
              key={item.id}
              className="flex items-center gap-2 text-xs px-1 py-0.5 rounded hover:bg-bg-secondary cursor-pointer"
              onClick={() => navigate({ to: "/autoruns", search: { imagePath: entry.exe! } })}
            >
              <span className="shrink-0 px-1 py-0.5 rounded bg-bg-elev-2 text-fg-secondary select-none">{item.category}</span>
              <span className="truncate">{item.entry}</span>
              <span className="font-mono text-fg-tertiary truncate ml-auto">{item.location}</span>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-xs text-fg-tertiary">{t("process.association.no-autoruns")}</div>
      )}
      <button
        className="text-xs text-accent hover:underline select-none"
        onClick={() => navigate({ to: "/autoruns", search: { imagePath: entry.exe! } })}
      >
        {t("process.association.view-autoruns")} →
      </button>
    </div>
  );
}

function SysmonTab({ entry }: { entry: ProcessEntry }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const allEvents = useLogCollectorStore((s) => s.events);

  const filteredEvents = useMemo(
    () => allEvents.filter((e: SysmonEvent) => e.process_id === entry.pid),
    [allEvents, entry.pid]
  );

  return (
    <div className="space-y-2">
      {filteredEvents.length > 0 ? (
        <div className="space-y-1">
          {filteredEvents.slice(0, 50).map((event: SysmonEvent, i: number) => {
            const eventType = event.event_type as ExtendedSysmonEventType;
            const label = EVENT_TYPE_LABELS[eventType] ?? event.event_type;
            const colorClass = EVENT_TYPE_COLORS[eventType] ?? "bg-gray-500/15 text-gray-500 border-gray-500/25";
            return (
              <div key={i} className="flex items-center gap-2 text-xs px-1 py-0.5 rounded hover:bg-bg-secondary">
                <span className={`shrink-0 px-1 py-0.5 rounded border select-none ${colorClass}`}>
                  {label}
                </span>
                {event.event_type === "network_connect" && (
                  <span className="font-mono truncate">
                    {event.source_ip}:{event.source_port} → {event.destination_ip}:{event.destination_port}
                  </span>
                )}
                {event.event_type === "dns" && (
                  <span className="font-mono truncate">{event.query_name}</span>
                )}
                {(event.event_type === "dns_client") && (
                  <span className="font-mono truncate">{event.query_name}</span>
                )}
                {event.event_type === "process_create" && (
                  <span className="font-mono truncate">{event.process_path}</span>
                )}
                {!["network_connect", "dns", "dns_client", "process_create"].includes(event.event_type) && (
                  <span className="truncate">{event.process_name}</span>
                )}
                <span className="font-mono text-fg-tertiary ml-auto shrink-0">{event.timestamp.slice(11, 19)}</span>
              </div>
            );
          })}
          {filteredEvents.length > 50 && (
            <div className="text-xs text-fg-tertiary select-none">
              {t("process.sysmon.more", { count: filteredEvents.length - 50 })}
            </div>
          )}
        </div>
      ) : (
        <div className="text-xs text-fg-tertiary">{t("process.association.no-sysmon")}</div>
      )}
      <button
        className="text-xs text-accent hover:underline select-none"
        onClick={() => navigate({ to: "/log-collector" })}
      >
        {t("process.association.view-sysmon")} →
      </button>
    </div>
  );
}

export function ProcessDetail({ entry, snapshotTime, onClose }: Props) {
  const { t } = useTranslation();
  const chainQuery = useProcessChain(entry?.pid ?? null);
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("chain");

  // Fetch network/autoruns data for count badges (only when entry exists)
  const networkQuery = useNetworkByPid(entry?.pid ?? null);
  const autorunsQuery = useAutorunsByPath(entry?.exe ?? null);
  const sysmonEvents = useLogCollectorStore((s) => s.events);
  const sysmonCount = entry ? sysmonEvents.filter((e: SysmonEvent) => e.process_id === entry.pid).length : 0;

  if (!entry) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("process.detail.select-row")}
      </div>
    );
  }

  const chain = chainQuery.data;
  const isLoading = chainQuery.isLoading;

  const tabs: { id: TabId; labelKey: string; count?: number }[] = [
    { id: "chain", labelKey: "process.tabs.chain" },
    { id: "network", labelKey: "process.tabs.network", count: networkQuery.data?.length },
    { id: "sysmon", labelKey: "process.tabs.sysmon", count: sysmonCount },
    { id: "autoruns", labelKey: "process.tabs.autoruns", count: autorunsQuery.data?.length },
  ];

  return (
    <div className="h-full overflow-auto p-4 space-y-3">
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-sm font-medium text-fg-primary">{entry.name}</div>
          <div className="flex items-center gap-1.5 mt-1 text-xs text-fg-tertiary flex-wrap">
            <span className="font-mono">PID {entry.pid}</span>
            <span>·</span>
            <span className="font-mono">PPID {entry.ppid}</span>
            {entry.is_suspicious && (
              <>
                <span>·</span>
                <span className="text-warning flex items-center gap-0.5">
                  <AlertTriangle className="h-3 w-3" />{entry.suspicious_reason}
                </span>
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

      {entry.exe && (
        <div className="text-xs">
          <div className="text-fg-tertiary">{t("process.detail.path")}</div>
          <div className="font-mono break-all">{entry.exe}</div>
        </div>
      )}

      {/* Tab bar */}
      <div className="flex gap-1">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={`text-xs px-2 py-1 rounded select-none ${
              activeTab === tab.id
                ? "bg-bg-elev-2 text-fg-primary"
                : "text-fg-tertiary hover:text-fg-secondary"
            }`}
            onClick={() => setActiveTab(tab.id)}
          >
            {t(tab.labelKey)}
            {tab.count !== undefined && (
              <span className="ml-1 text-fg-tertiary">({tab.count})</span>
            )}
          </button>
        ))}
      </div>

      <Separator />

      {/* Tab content */}
      {activeTab === "chain" && (
        <div className="space-y-1">
          {isLoading ? (
            <div className="text-xs text-fg-tertiary">{t("common.loading")}</div>
          ) : chain && chain.nodes.length > 0 ? (
            <div className="space-y-0.5 pl-2">
              {[...chain.nodes].reverse().map((node, i) => {
                const originalIdx = chain.nodes.length - 1 - i;
                const isExpanded = expandedIdx === originalIdx;
                return (
                  <div key={node.pid}>
                    <div
                      className={`flex items-center gap-1 text-xs cursor-pointer rounded px-1 py-0.5 -mx-1 hover:bg-bg-secondary ${node.is_target ? "bg-accent/15 text-accent" : ""}`}
                      style={{ paddingLeft: `${i * 12 + 4}px` }}
                      onClick={() => setExpandedIdx(isExpanded ? null : originalIdx)}
                    >
                      <span className="text-fg-tertiary">└─</span>
                      <ChainNodeIcon imagePath={node.exe} />
                      <span className={node.is_target ? "text-fg-primary font-medium" : "text-fg-secondary"}>
                        {node.name} ({node.pid})
                      </span>
                      {node.is_suspicious && (
                        <span className="text-[10px] text-warning">⚠ {node.suspicious_reason}</span>
                      )}
                    </div>
                    {isExpanded && <ChainNodeDetail node={chain.nodes[originalIdx]} />}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="text-xs text-fg-tertiary">{t("process.chain.empty")}</div>
          )}
        </div>
      )}

      {activeTab === "network" && <NetworkTab entry={entry} />}

      {activeTab === "sysmon" && <SysmonTab entry={entry} />}

      {activeTab === "autoruns" && <AutorunsTab entry={entry} />}

      {snapshotTime && (
        <>
          <Separator />
          <div className="text-xs text-fg-tertiary select-none">
            {t("process.detail.snapshot-time")}: {snapshotTime}
          </div>
        </>
      )}
    </div>
  );
}
