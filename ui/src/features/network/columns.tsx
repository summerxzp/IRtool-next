import type { ColumnDef } from "@tanstack/react-table";
import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { formatEpochSeconds } from "@/lib/utils";
import type { NetConn, ConnState, CmdlineStatus } from "./types";

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

function CmdlineStatusIcon({ status }: { status: CmdlineStatus }) {
  switch (status) {
    case "pending":
      return (
        <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-fg-tertiary border-t-transparent" title="Pending" />
      );
    case "ready":
      return (
        <span className="inline-block h-3 w-3 text-success" title="Ready">✓</span>
      );
    case "denied":
      return (
        <span className="inline-block h-3 w-3 text-warning" title="Access Denied">⊘</span>
      );
    case "exited":
      return (
        <span className="inline-block h-3 w-3 text-fg-tertiary" title="Process Exited">✗</span>
      );
    case "failed":
      return (
        <span className="inline-block h-3 w-3 text-danger" title="Failed">✗</span>
      );
    default:
      return null;
  }
}

export const networkColumns: ColumnDef<NetConn>[] = [
  {
    id: "first_seen",
    accessorFn: (r) => r.first_seen,
    header: "First Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{formatEpochSeconds(row.original.first_seen)}</span>
    ),
  },
  {
    id: "pid",
    accessorFn: (r) => r.pid,
    header: "PID",
    size: 55,
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
    id: "cmdline",
    accessorFn: (r) => r.process_cmdline ?? "",
    header: "Cmdline",
    size: 200,
    cell: ({ row }) => {
      const val = row.original.process_cmdline;
      const status = row.original.cmdline_status;
      return (
        <span className="flex items-center gap-1">
          <CmdlineStatusIcon status={status} />
          {!val && status !== "pending" ? (
            <span className="text-fg-tertiary">-</span>
          ) : (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="truncate max-w-[180px] block cursor-default font-mono text-xs">
                    {val ?? ""}
                  </span>
                </TooltipTrigger>
                {val && (
                  <TooltipContent side="bottom" className="max-w-md break-all font-mono text-xs">
                    {val}
                  </TooltipContent>
                )}
              </Tooltip>
            </TooltipProvider>
          )}
        </span>
      );
    },
  },
  {
    id: "last_seen",
    accessorFn: (r) => r.last_seen,
    header: "Last Seen",
    size: 160,
    cell: ({ row }) => (
      <span className="font-mono text-xs">{formatEpochSeconds(row.original.last_seen)}</span>
    ),
  },
];
