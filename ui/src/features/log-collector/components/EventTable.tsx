import { useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";

import { DataTable } from "@/components/data-table/DataTable";
import { type ColumnDef } from "@tanstack/react-table";
import { formatEventTimestamp } from "@/lib/utils";

import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS, EVENT_TYPE_SEVERITY, severityToBadgeClass } from "../types";
import type { SysmonEvent, ExtendedSysmonEventType } from "../types";

interface Props {
  events: SysmonEvent[];
}

function getDestination(event: SysmonEvent): string {
  const et = event.event_type as ExtendedSysmonEventType;
  switch (et) {
    case "network_connect":
      return `${event.destination_ip}:${event.destination_port}`;
    case "dns":
    case "dns_client":
    case "tls_sni":
    case "dns_pcap":
      return event.query_name || "-";
    case "create_remote_thread":
      return `${event.source_process_name} → ${event.target_process_name}`;
    case "file_create":
      return event.target_filename || "-";
    default:
      return event.process_name || "-";
  }
}

function getPath(event: SysmonEvent): string {
  const et = event.event_type as ExtendedSysmonEventType;
  switch (et) {
    case "network_connect":
    case "dns":
    case "dns_client":
    case "file_create":
      return event.process_path || "";
    case "tls_sni":
    case "dns_pcap":
      return "";
    case "create_remote_thread":
      return event.source_process_path || "";
    default:
      return event.process_path || "";
  }
}


export function EventTable({ events }: Props) {
  const { t } = useTranslation();
  const { filters, selectedEvent, setSelectedEvent } = useLogCollectorStore();

  // Filter events — keep stable reference
  const filtered = useMemo(() => {
    return events.filter((e) => {
      if (filters.eventTypes.length > 0 && !filters.eventTypes.includes(e.event_type as ExtendedSysmonEventType)) return false;
      if (filters.externalOnly && !e.is_external) return false;
      if (filters.search) {
        const q = filters.search.toLowerCase();
        const searchable = `${e.process_name} ${e.query_name} ${e.destination_ip} ${e.destination_port} ${e.source_ip} ${e.source_port} ${e.target_filename} ${e.source_process_name} ${e.target_process_name}`.toLowerCase();
        if (!searchable.includes(q)) return false;
      }
      return true;
    });
  }, [events, filters]);

  const columns = useMemo<ColumnDef<SysmonEvent, unknown>[]>(() => [
    {
      accessorKey: "timestamp",
      header: t("log-collector.table.time"),
      size: 144,
      cell: ({ getValue }) => {
        const ts = getValue() as string | number;
        const display = typeof ts === 'number' ? formatEventTimestamp(ts) : (ts || "-");
        return <span className="font-mono text-fg-secondary whitespace-nowrap overflow-hidden text-ellipsis">{display}</span>;
      },
    },
    {
      accessorKey: "event_type",
      header: t("log-collector.table.type"),
      size: 96,
      cell: ({ row }) => {
        const et = row.original.event_type as ExtendedSysmonEventType;
        return <div className="flex items-center gap-1">
          <span className={`inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap ${severityToBadgeClass(EVENT_TYPE_SEVERITY[et])}`}>
            {EVENT_TYPE_LABELS[et] || et}
          </span>
          {row.original.is_external && (
            <span className="inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap bg-info-bg text-accent border border-info-border">
              {t("log-collector.table.external")}
            </span>
          )}
        </div>;
      },
    },
    {
      id: "destination",
      accessorFn: (row) => getDestination(row),
      header: t("log-collector.table.destination"),
      size: 176,
      cell: ({ row }) => <span className="truncate text-fg-primary" title={getDestination(row.original)}>{getDestination(row.original)}</span>,
    },
    {
      id: "path",
      accessorFn: (row) => getPath(row),
      header: t("log-collector.table.path"),
      size: 300,
      cell: ({ row }) => <span className="truncate text-fg-secondary" title={getPath(row.original)}>{getPath(row.original)}</span>,
    },
  ], [t]);

  const handleDoubleClick = useCallback((event: SysmonEvent) => {
    const value = getDestination(event);
    if (value && value !== "-") navigator.clipboard.writeText(value);
  }, []);

  return (
    <DataTable
      columns={columns}
      data={filtered}
      getRowId={(e) => `${e.record_id}-${e.timestamp}`}
      onRowSelect={setSelectedEvent}
      onRowDoubleClick={handleDoubleClick}
      selectedRowId={selectedEvent ? `${selectedEvent.record_id}-${selectedEvent.timestamp}` : null}
      empty={t("log-collector.table.no-events")}
      persistKey="log-collector"
    />
  );
}
