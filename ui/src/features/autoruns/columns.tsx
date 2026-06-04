import type { ColumnDef } from "@tanstack/react-table";
import { SignatureBadge } from "./components/SignatureBadge";
import type { AutorunItem } from "./types";

export const autorunsColumns: ColumnDef<AutorunItem>[] = [
  {
    id: "enabled",
    accessorFn: (r) => r.enabled,
    header: "启用",
    size: 50,
    cell: ({ row }) =>
      row.original.enabled ? (
        <span className="text-success text-xs">✓</span>
      ) : (
        <span className="text-fg-tertiary text-xs">✗</span>
      ),
  },
  {
    id: "signature",
    accessorFn: (r) => r.signature.kind,
    header: "签名",
    size: 80,
    cell: ({ row }) => <SignatureBadge status={row.original.signature} />,
  },
  {
    id: "entry",
    accessorFn: (r) => r.entry,
    header: "条目",
    size: 180,
    cell: ({ row }) => <span className="font-medium text-fg-primary">{row.original.entry}</span>,
  },
  {
    id: "image_path",
    accessorFn: (r) => r.image_path ?? "",
    header: "文件路径",
    size: 300,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary">{row.original.image_path ?? ""}</span>
    ),
  },
  {
    id: "publisher",
    accessorFn: (r) => r.publisher,
    header: "发布者",
    size: 160,
  },
  {
    id: "location",
    accessorFn: (r) => r.location,
    header: "位置",
    size: 280,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-tertiary">{row.original.location}</span>
    ),
  },
];
