import { DataTable } from "@/components/data-table/DataTable";
import { useHistoryColumns } from "../columns";
import type { RecentActivity } from "../types";

interface Props {
  data: RecentActivity[];
  search: string;
}

// url + visit_time 在同一秒内多次访问同 URL 时会重复，
// 因此追加递增索引兜底，避免 React key 冲突与 react-table getRow 找不到行崩溃。
function rowKey(row: RecentActivity, index: number) {
  return `${row.url}|${row.visit_time}|${index}`;
}

export function HistoryTable({ data, search }: Props) {
  const columns = useHistoryColumns();

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
      persistKey="browser-forensics-history"
      density="compact"
    />
  );
}
