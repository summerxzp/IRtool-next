import { useMemo } from "react";
import { DataTable } from "@/components/data-table/DataTable";
import { autorunsColumns } from "../columns";
import { useAutorunsStore } from "../store";
import type { AutorunItem } from "../types";

interface Props {
  data: AutorunItem[];
  onRowSelect: (row: AutorunItem | null) => void;
  onRowContextMenu?: (row: AutorunItem, event: React.MouseEvent) => void;
}

function rowKey(row: AutorunItem) {
  return String(row.id);
}

function rowClassName(row: AutorunItem) {
  if (!row.enabled) return "opacity-60";
  return undefined;
}

export function AutorunsTable({ data, onRowSelect, onRowContextMenu }: Props) {
  const filters = useAutorunsStore((s) => s.filters);

  const filtered = useMemo(() => {
    let result = data;
    if (filters.status === "enabled") {
      result = result.filter((r) => r.enabled);
    } else if (filters.status === "disabled") {
      result = result.filter((r) => !r.enabled);
    }
    if (filters.signature !== "all") {
      result = result.filter((r) => r.signature.kind === filters.signature);
    }
    if (filters.category !== "all") {
      result = result.filter((r) => r.category === filters.category);
    }
    if (filters.search.trim()) {
      const q = filters.search.toLowerCase();
      result = result.filter((r) => {
        const blob = `${r.entry} ${r.image_path ?? ""} ${r.launch_string ?? ""} ${r.publisher} ${r.location} ${r.category}`.toLowerCase();
        return blob.includes(q);
      });
    }
    return result;
  }, [data, filters]);

  return (
    <DataTable
      columns={autorunsColumns}
      data={filtered}
      getRowId={rowKey}
      rowClassName={rowClassName}
      onRowSelect={onRowSelect}
      onRowContextMenu={onRowContextMenu}
      persistKey="autoruns"
      density="compact"
    />
  );
}
