import { useMemo } from "react";
import { DataTable } from "@/components/data-table/DataTable";
import { networkColumns } from "../columns";
import { useNetworkStore } from "../store";
import type { NetConn } from "../types";

interface Props {
  data: NetConn[];
  onRowSelect: (row: NetConn | null) => void;
  onRowContextMenu?: (row: NetConn, event: React.MouseEvent) => void;
  selectedRowId?: string | null;
}

function rowKey(row: NetConn) {
  return `${row.proto}|${row.family}|${row.local.addr}:${row.local.port}|${row.remote.addr}:${row.remote.port}|${row.pid}`;
}

function rowClassName(row: NetConn) {
  if (!row.is_current) return "opacity-60";
  return undefined;
}

export function NetworkTable({ data, onRowSelect, onRowContextMenu, selectedRowId }: Props) {
  const { filters } = useNetworkStore();

  const filtered = useMemo(() => {
    let result = data;
    if (!filters.showHistory) {
      result = result.filter((r) => r.is_current);
    }
    if (filters.proto !== "all") {
      result = result.filter((r) => r.proto === filters.proto);
    }
    if (filters.states.length > 0) {
      result = result.filter((r) => filters.states.includes(r.state));
    }
    if (filters.search.trim()) {
      const q = filters.search.toLowerCase();
      result = result.filter((r) => {
        const blob =
          `${r.pid} ${r.process_name ?? ""} ${r.process_path ?? ""} ${r.local.addr}:${r.local.port} ${r.remote.addr}:${r.remote.port}`.toLowerCase();
        return blob.includes(q);
      });
    }
    return result;
  }, [data, filters]);

  return (
    <DataTable
      columns={networkColumns}
      data={filtered}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      onRowContextMenu={onRowContextMenu}
      persistKey="network"
      density="compact"
      selectedRowId={selectedRowId}
    />
  );
}
