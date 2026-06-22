import { DataTable } from "@/components/data-table/DataTable";
import { processColumns } from "../columns";
import type { ProcessEntry } from "../types";

interface Props {
  data: ProcessEntry[];
  onRowSelect: (row: ProcessEntry | null) => void;
  selectedRowId?: string | null;
}

function rowKey(row: ProcessEntry) {
  return String(row.pid);
}

function rowClassName(row: ProcessEntry) {
  if (row.is_suspicious) return "bg-warning/8";
  return undefined;
}

export function ProcessTable({ data, onRowSelect, selectedRowId }: Props) {
  return (
    <DataTable
      columns={processColumns}
      data={data}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      persistKey="process"
      density="compact"
      selectedRowId={selectedRowId}
    />
  );
}
