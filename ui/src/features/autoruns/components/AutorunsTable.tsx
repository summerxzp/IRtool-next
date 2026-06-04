import { DataTable } from "@/components/data-table/DataTable";
import { autorunsColumns } from "../columns";
import type { AutorunItem } from "../types";

interface Props {
  data: AutorunItem[];
  onRowSelect: (row: AutorunItem | null) => void;
  onRowContextMenu?: (row: AutorunItem, event: React.MouseEvent) => void;
  selectedRowId?: string | null;
}

function rowKey(row: AutorunItem) {
  return String(row.id);
}

function rowClassName(row: AutorunItem) {
  if (!row.file_exists) return "bg-yellow-500/10";
  if (row.signature.kind === "unsigned") return "bg-red-500/8";
  if (!row.enabled) return "opacity-60";
  return undefined;
}

export function AutorunsTable({ data, onRowSelect, onRowContextMenu, selectedRowId }: Props) {
  return (
    <DataTable
      columns={autorunsColumns}
      data={data}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      onRowContextMenu={onRowContextMenu}
      persistKey="autoruns"
      density="compact"
      selectedRowId={selectedRowId}
    />
  );
}
