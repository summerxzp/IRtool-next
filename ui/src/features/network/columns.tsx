import type { ColumnDef } from "@tanstack/react-table";
import { Badge } from "@/components/ui/badge";
import type { NetConn, ConnState } from "./types";

const STATE_VARIANT: Partial<Record<ConnState, "default" | "success" | "warning" | "danger" | "info">> = {
  ESTABLISHED: "success",
  LISTEN: "info",
  TIME_WAIT: "warning",
  CLOSE_WAIT: "danger",
};

function fmtAddr(addr: string) {
  if (!addr || addr === "0.0.0.0" || addr === "::") return "*";
  return addr;
}

function fmtPort(port: number) {
  return port === 0 ? "*" : String(port);
}

function fmtTime(epoch: number) {
  if (!epoch) return "-";
  const d = new Date(epoch * 1000);
  return d.toLocaleString("en-GB", { hour12: false });
}

export const networkColumns: ColumnDef<NetConn>[] = [
  {
    id: "proto",
    accessorFn: (r) => r.proto,
    header: "Proto",
    size: 60,
    cell: ({ row }) => row.original.proto.toUpperCase(),
  },
  {
    id: "family",
    accessorFn: (r) => r.family,
    header: "Fam",
    size: 50,
    cell: ({ row }) => row.original.family.toUpperCase(),
  },
  {
    id: "local",
    accessorFn: (r) => `${r.local.addr}:${r.local.port}`,
    header: "Local",
    size: 200,
    cell: ({ row }) =>
      `${fmtAddr(row.original.local.addr)}:${fmtPort(row.original.local.port)}`,
  },
  {
    id: "remote",
    accessorFn: (r) => `${r.remote.addr}:${r.remote.port}`,
    header: "Remote",
    size: 200,
    cell: ({ row }) =>
      `${fmtAddr(row.original.remote.addr)}:${fmtPort(row.original.remote.port)}`,
  },
  {
    id: "state",
    accessorFn: (r) => r.state,
    header: "State",
    size: 110,
    cell: ({ row }) => {
      const s = row.original.state;
      if (!s || s === "NONE") return <span className="text-fg-tertiary">-</span>;
      const variant = STATE_VARIANT[s] ?? "default";
      return <Badge variant={variant}>{s}</Badge>;
    },
  },
  {
    id: "pid",
    accessorFn: (r) => r.pid,
    header: "PID",
    size: 70,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{row.original.pid}</span>
    ),
  },
  {
    id: "process",
    accessorFn: (r) => r.process_name ?? "",
    header: "Process",
    size: 160,
    cell: ({ row }) => row.original.process_name ?? "",
  },
  {
    id: "path",
    accessorFn: (r) => r.process_path ?? "",
    header: "Path",
    size: 280,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary">
        {row.original.process_path ?? ""}
      </span>
    ),
  },
  {
    id: "first_seen",
    accessorFn: (r) => r.first_seen,
    header: "First Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{fmtTime(row.original.first_seen)}</span>
    ),
  },
  {
    id: "last_seen",
    accessorFn: (r) => r.last_seen,
    header: "Last Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{fmtTime(row.original.last_seen)}</span>
    ),
  },
];
