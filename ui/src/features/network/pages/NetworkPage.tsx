import { useMemo, useState } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useClearHistory, useKillProcess, useNetwork } from "../hooks";
import { NetworkToolbar } from "../components/NetworkToolbar";
import { NetworkTable } from "../components/NetworkTable";
import { NetworkDetail } from "../components/NetworkDetail";
import { NetworkStatsBar } from "../components/NetworkStatsBar";
import { KillProcessDialog } from "../components/KillProcessDialog";
import { exportCsv } from "@/lib/csv";
import type { NetConn } from "../types";

export function NetworkPage() {
  const { t } = useTranslation();
  const query = useNetwork();
  const killMutation = useKillProcess();
  const clearMutation = useClearHistory();
  const [selected, setSelected] = useState<NetConn | null>(null);
  const [killDialogOpen, setKillDialogOpen] = useState(false);
  const [contextRow, setContextRow] = useState<NetConn | null>(null);
  const [contextPos, setContextPos] = useState<{ x: number; y: number } | null>(null);

  const data = useMemo(() => query.data?.connections ?? [], [query.data]);

  const handleExport = async () => {
    await exportCsv(
      data.map((c) => ({
        proto: c.proto,
        family: c.family,
        local_addr: c.local.addr,
        local_port: c.local.port,
        remote_addr: c.remote.addr,
        remote_port: c.remote.port,
        state: c.state,
        pid: c.pid,
        process_name: c.process_name,
        process_path: c.process_path,
        first_seen: new Date(c.first_seen * 1000).toISOString(),
        last_seen: new Date(c.last_seen * 1000).toISOString(),
        is_current: c.is_current,
      })),
      [
        "proto", "family", "local_addr", "local_port", "remote_addr",
        "remote_port", "state", "pid", "process_name", "process_path",
        "first_seen", "last_seen", "is_current",
      ],
      `irtool-network-${Date.now()}.csv`,
    );
  };

  const handleKill = (pid: number) => {
    killMutation.mutate(pid);
  };

  const handleContextMenu = (row: NetConn, event: React.MouseEvent) => {
    event.preventDefault();
    setContextRow(row);
    setContextPos({ x: event.clientX, y: event.clientY });
  };

  return (
    <div className="h-full flex flex-col">
      <NetworkToolbar
        onExport={handleExport}
        onClearHistory={() => clearMutation.mutate()}
        onKillSelected={() => {
          if (selected) setKillDialogOpen(true);
        }}
        hasSelection={selected != null}
        loading={query.isFetching}
      />

      <div className="flex-1 min-h-0">
        <Group orientation="horizontal">
          <Panel defaultSize={70} minSize={40}>
            <NetworkTable
              data={data}
              onRowSelect={setSelected}
              onRowContextMenu={handleContextMenu}
            />
          </Panel>
          <Separator className="w-px bg-border hover:bg-accent transition-colors" />
          <Panel defaultSize={30} minSize={20}>
            <NetworkDetail conn={selected} />
          </Panel>
        </Group>
      </div>

      <NetworkStatsBar data={data} />

      <KillProcessDialog
        conn={selected}
        open={killDialogOpen}
        onOpenChange={setKillDialogOpen}
        onConfirm={handleKill}
      />

      {contextRow && contextPos && (
        <DropdownMenu open={true} onOpenChange={() => setContextRow(null)}>
          <DropdownMenuTrigger asChild>
            <span
              className="fixed"
              style={{ top: contextPos.y, left: contextPos.x, width: 0, height: 0 }}
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem
              onClick={() => {
                navigator.clipboard.writeText(
                  `${contextRow.proto.toUpperCase()} ${contextRow.local.addr}:${contextRow.local.port} -> ${contextRow.remote.addr}:${contextRow.remote.port} pid=${contextRow.pid} ${contextRow.process_name ?? ""}`,
                );
                setContextRow(null);
              }}
            >
              {t("network.context-menu.copy-row")}
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                setSelected(contextRow);
                setKillDialogOpen(true);
                setContextRow(null);
              }}
            >
              {t("network.context-menu.kill")}
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled
              onClick={() => {
                setContextRow(null);
              }}
            >
              {t("network.context-menu.search-workspace")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}
