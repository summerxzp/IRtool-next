import type { ColumnDef } from "@tanstack/react-table";
import type { ProcessEntry } from "./types";

export const processColumns: ColumnDef<ProcessEntry>[] = [
  {
    id: "pid",
    accessorFn: (r) => r.pid,
    header: "PID",
    size: 70,
    cell: ({ row }) => <span className="font-mono text-xs">{row.original.pid}</span>,
  },
  {
    id: "ppid",
    accessorFn: (r) => r.ppid,
    header: "PPID",
    size: 70,
    cell: ({ row }) => <span className="font-mono text-xs">{row.original.ppid}</span>,
  },
  {
    id: "name",
    accessorFn: (r) => r.name,
    header: "名称",
    size: 160,
    cell: ({ row }) => (
      <span className={`font-medium ${row.original.is_suspicious ? "text-warning" : "text-fg-primary"}`}>
        {row.original.name}
      </span>
    ),
  },
  {
    id: "exe",
    accessorFn: (r) => r.exe ?? "",
    header: "路径",
    size: 350,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary truncate">{row.original.exe ?? ""}</span>
    ),
  },
  {
    id: "suspicious",
    accessorFn: (r) => r.is_suspicious,
    header: "可疑",
    size: 80,
    cell: ({ row }) =>
      row.original.is_suspicious ? (
        <span className="text-warning text-xs" title={row.original.suspicious_reason ?? undefined}>⚠</span>
      ) : (
        <span className="text-fg-tertiary text-xs">—</span>
      ),
  },
];
