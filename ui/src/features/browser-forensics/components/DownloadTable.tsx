import { DataTable } from "@/components/data-table/DataTable";
import { useDownloadColumns } from "../columns";
import type { DownloadInfo } from "../types";

interface Props {
  data: DownloadInfo[];
  search: string;
}

// local_path + start_time 可能重复（同文件多次下载），追加索引兜底
function rowKey(row: DownloadInfo, index: number) {
  return `${row.local_path}|${row.start_time ?? ""}|${index}`;
}

export function DownloadTable({ data, search }: Props) {
  const columns = useDownloadColumns();

  const filtered = search.trim()
    ? data.filter((r) => {
        const q = search.toLowerCase();
        return (
          r.filename.toLowerCase().includes(q) ||
          r.download_url.toLowerCase().includes(q) ||
          r.local_path.toLowerCase().includes(q)
        );
      })
    : data;

  return (
    <DataTable
      columns={columns}
      data={filtered}
      getRowId={rowKey}
      persistKey="browser-forensics-downloads"
      density="compact"
    />
  );
}
