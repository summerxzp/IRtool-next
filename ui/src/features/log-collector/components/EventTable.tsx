import { useEffect, useRef, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS, EVENT_TYPE_COLORS } from "../types";
import type { SysmonEvent } from "../types";

interface Props {
  events: SysmonEvent[];
}

function getEventSummary(event: SysmonEvent): string {
  switch (event.event_type) {
    case "dns":
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

export function EventTable({ events }: Props) {
  const { t } = useTranslation();
  const { filters, selectedRecordId, setSelectedRecordId, autoScroll, setAutoScroll } = useLogCollectorStore();
  const containerRef = useRef<HTMLDivElement>(null);

  // Filter events
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

  // Auto-scroll
  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
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

  return (
    <div ref={containerRef} onScroll={handleScroll} className="flex-1 overflow-auto">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-36 text-xs">{t("log-collector.table.time")}</TableHead>
            <TableHead className="w-24 text-xs">{t("log-collector.table.type")}</TableHead>
            <TableHead className="w-32 text-xs">{t("log-collector.table.process")}</TableHead>
            <TableHead className="text-xs">{t("log-collector.table.detail")}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {filtered.map((event, idx) => {
            const rid = event.record_id ?? idx;
            const isSelected = selectedRecordId === rid;
            return (
              <TableRow
                key={`${event.record_id ?? idx}-${idx}`}
                className={`cursor-pointer text-xs ${isSelected ? "bg-accent/50" : ""}`}
                onClick={() => setSelectedRecordId(rid)}
                onDoubleClick={() => handleDoubleClick(event)}
              >
                <TableCell className="font-mono text-fg-secondary whitespace-nowrap">
                  {event.timestamp || "-"}
                </TableCell>
                <TableCell>
                  <div className="flex items-center gap-1">
                    <Badge variant="outline" className={`text-[10px] px-1.5 py-0 ${EVENT_TYPE_COLORS[event.event_type]}`}>
                      {EVENT_TYPE_LABELS[event.event_type]}
                    </Badge>
                    {event.is_external && (
                      <Badge variant="outline" className="text-[10px] px-1.5 py-0 bg-blue-500/15 text-blue-500 border-blue-500/25">
                        {t("log-collector.table.external")}
                      </Badge>
                    )}
                  </div>
                </TableCell>
                <TableCell className="truncate max-w-32" title={event.process_path || event.source_process_name}>
                  {event.event_type === "create_remote_thread"
                    ? `${event.source_process_name} → ${event.target_process_name}`
                    : event.process_name || "-"}
                </TableCell>
                <TableCell className="truncate" title={getEventSummary(event)}>
                  {getEventSummary(event)}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
      {filtered.length === 0 && (
        <div className="flex items-center justify-center h-32 text-xs text-fg-tertiary">
          {t("log-collector.table.no-events")}
        </div>
      )}
    </div>
  );
}
