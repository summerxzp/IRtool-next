import { DataTable } from "@/components/data-table/DataTable";
import { useExtensionColumns } from "../columns";
import type { ExtensionInfo } from "../types";

interface Props {
  data: ExtensionInfo[];
  onRowSelect: (row: ExtensionInfo | null) => void;
  selectedRowId?: string | null;
  search: string;
}

function rowKey(row: ExtensionInfo) {
  return row.id;
}

function rowClassName(row: ExtensionInfo) {
  if (row.risk_flags.includes("high_privilege_combo")) return "bg-red-500/8";
  if (row.risk_flags.length > 0) return "bg-yellow-500/8";
  if (!row.enabled) return "opacity-60";
  return undefined;
}

export function ExtensionTable({ data, onRowSelect, selectedRowId, search }: Props) {
  const columns = useExtensionColumns();

  const filtered = search.trim()
    ? data.filter((r) => {
        const q = search.toLowerCase();
        return (
          r.name.toLowerCase().includes(q) ||
          r.id.toLowerCase().includes(q) ||
          r.description?.toLowerCase().includes(q) ||
          r.permissions.some((p) => p.toLowerCase().includes(q))
        );
      })
    : data;

  return (
    <DataTable
      columns={columns}
      data={filtered}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      persistKey="browser-forensics-extensions"
      density="compact"
      selectedRowId={selectedRowId}
    />
  );
}
