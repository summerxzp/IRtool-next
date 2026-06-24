import { DataTable } from "@/components/data-table/DataTable";
import { useTabColumns } from "../columns";
import type { RecoveredTab } from "../types";

interface Props {
  data: RecoveredTab[];
  search: string;
}

function rowKey(row: RecoveredTab) {
  return `${row.url}-${row.tab_index ?? ""}`;
}

export function TabTable({ data, search }: Props) {
  const columns = useTabColumns();

  const filtered = search.trim()
    ? data.filter((r) => {
        const q = search.toLowerCase();
        return (
          r.url.toLowerCase().includes(q) ||
          r.title.toLowerCase().includes(q)
        );
      })
    : data;

  return (
    <DataTable
      columns={columns}
      data={filtered}
      getRowId={rowKey}
      persistKey="browser-forensics-tabs"
      density="compact"
    />
  );
}
