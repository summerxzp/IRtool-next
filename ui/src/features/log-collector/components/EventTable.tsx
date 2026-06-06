import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS, EVENT_TYPE_COLORS } from "../types";
import type { SysmonEvent } from "../types";

interface Props {
  events: SysmonEvent[];
}

function getEventSummary(event: SysmonEvent): string {
  switch (event.event_type) {
    case "dns":
    case "dns_client":
      return event.query_name || "-";
    case "network_connect":
      return `${event.destination_ip}:${event.destination_port}`;
    case "create_remote_thread":
      return `${event.source_process_name} → ${event.target_process_name}`;
    case "file_create":
      return event.target_filename || "-";
    default:
      return "-";
  }
}

function getCopyValue(event: SysmonEvent): string {
  switch (event.event_type) {
    case "dns":
    case "dns_client":
      return event.query_name;
    case "network_connect":
      return `${event.destination_ip}:${event.destination_port}`;
    case "create_remote_thread":
      return event.start_address;
    case "file_create":
      return event.target_filename;
    default:
      return "";
  }
}

/** Generate a unique stable key for an event row */
function getEventKey(event: SysmonEvent, idx: number): string {
  return `${event.event_id}-${event.timestamp}-${event.record_id ?? idx}-${event.process_id}-${event.source_process_id}-${event.target_process_id}`;
}

export function EventTable({ events }: Props) {
  const { t } = useTranslation();
  const { filters, selectedEvent, setSelectedEvent, autoScroll, setAutoScroll } = useLogCollectorStore();
  const containerRef = useRef<HTMLDivElement>(null);

  // Filter events — keep stable reference
  const filtered = useMemo(() => {
    return events.filter((e) => {
      if (filters.eventType !== "all" && e.event_type !== filters.eventType) return false;
      if (filters.externalOnly && !e.is_external) return false;
      if (filters.search) {
        const q = filters.search.toLowerCase();
        const searchable = `${e.process_name} ${e.query_name} ${e.destination_ip} ${e.target_filename} ${e.source_process_name} ${e.target_process_name}`.toLowerCase();
        if (!searchable.includes(q)) return false;
      }
      return true;
    });
  }, [events, filters]);

  // Auto-scroll — only when new events arrive AND autoScroll is on
  const prevLenRef = useRef(filtered.length);
  useEffect(() => {
    if (autoScroll && containerRef.current && filtered.length > prevLenRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
    prevLenRef.current = filtered.length;
  }, [filtered.length, autoScroll]);

  // Detect user scroll
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const atBottom = scrollHeight - scrollTop - clientHeight < 30;
    if (atBottom !== autoScroll) {
      setAutoScroll(atBottom);
    }
  }, [autoScroll, setAutoScroll]);

  const handleDoubleClick = useCallback((event: SysmonEvent) => {
    const value = getCopyValue(event);
    if (value) navigator.clipboard.writeText(value);
  }, []);

  // Virtual window — only render visible rows for performance
  const ITEM_HEIGHT = 28; // px, fixed row height
  const OVERSCAN = 10;
  const [scrollTop, setScrollTop] = useState(0);

  // Reset scroll position when filters change
  useEffect(() => {
    setScrollTop(0);
    if (containerRef.current) containerRef.current.scrollTop = 0;
  }, [filters]);

  const handleScrollVirtual = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
    handleScroll();
  }, [handleScroll]);

  const { startIdx, endIdx, paddingTop, paddingBottom } = useMemo(() => {
    const total = filtered.length;
    const start = Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - OVERSCAN);
    const end = Math.min(total, Math.ceil((scrollTop + (containerRef.current?.clientHeight ?? 600)) / ITEM_HEIGHT) + OVERSCAN);
    return {
      startIdx: start,
      endIdx: end,
      paddingTop: start * ITEM_HEIGHT,
      paddingBottom: Math.max(0, (total - end) * ITEM_HEIGHT),
    };
  }, [filtered.length, scrollTop]);

  return (
    <div ref={containerRef} onScroll={handleScrollVirtual} className="flex-1 overflow-auto">
      {/* Header */}
      <div className="sticky top-0 z-10 flex bg-bg-elev-2 border-b border-border text-[10px] text-fg-tertiary uppercase tracking-wider select-none">
        <div className="w-36 px-2 py-1 shrink-0">{t("log-collector.table.time")}</div>
        <div className="w-24 px-2 py-1 shrink-0">{t("log-collector.table.type")}</div>
        <div className="w-32 px-2 py-1 shrink-0">{t("log-collector.table.process")}</div>
        <div className="flex-1 px-2 py-1 min-w-0">{t("log-collector.table.detail")}</div>
      </div>

      {/* Virtualized rows */}
      <div style={{ paddingTop, paddingBottom }}>
        {filtered.slice(startIdx, endIdx).map((event, i) => {
          const actualIdx = startIdx + i;
          const isSelected = selectedEvent === event;
          return (
            <div
              key={getEventKey(event, actualIdx)}
              className={`flex items-center text-xs border-b border-border/50 hover:bg-bg-elev-2 cursor-pointer select-none ${isSelected ? "bg-bg-elev-2" : ""}`}
              style={{ height: ITEM_HEIGHT }}
              onClick={() => setSelectedEvent(event)}
              onDoubleClick={() => handleDoubleClick(event)}
            >
              <div className="w-36 px-2 shrink-0 font-mono text-fg-secondary whitespace-nowrap overflow-hidden text-ellipsis">
                {event.timestamp || "-"}
              </div>
              <div className="w-24 px-2 shrink-0">
                <div className="flex items-center gap-1">
                  <span className={`inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap ${EVENT_TYPE_COLORS[event.event_type]}`}>
                    {EVENT_TYPE_LABELS[event.event_type]}
                  </span>
                  {event.is_external && (
                    <span className="inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap bg-blue-500/15 text-blue-500 border border-blue-500/25">
                      {t("log-collector.table.external")}
                    </span>
                  )}
                </div>
              </div>
              <div className="w-32 px-2 shrink-0 truncate text-fg-primary" title={event.process_path || event.source_process_name}>
                {event.event_type === "create_remote_thread"
                  ? `${event.source_process_name} → ${event.target_process_name}`
                  : event.process_name || "-"}
              </div>
              <div className="flex-1 px-2 min-w-0 truncate text-fg-secondary" title={getEventSummary(event)}>
                {getEventSummary(event)}
              </div>
            </div>
          );
        })}
      </div>

      {filtered.length === 0 && (
        <div className="flex items-center justify-center h-32 text-xs text-fg-tertiary">
          {t("log-collector.table.no-events")}
        </div>
      )}
    </div>
  );
}
