import { useMemo, useState, useEffect } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { AlertTriangle } from "lucide-react";
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
import { useUIStore } from "@/stores/ui-store";
import { invoke } from "@tauri-apps/api/core";
import type { NetConn } from "../types";

// Background mode telemetry type
interface BackgroundTelemetry {
  events_written: number;
  events_dropped: number;
  last_event_at: number | null;
}

export function NetworkPage() {
  const { t } = useTranslation();
  const query = useNetwork();
  const killMutation = useKillProcess();
  const clearMutation = useClearHistory();
  const [selected, setSelected] = useState<NetConn | null>(null);
  const detailPosition = useUIStore((s) => s.detailPositions["network"] ?? "right");
  const [killDialogOpen, setKillDialogOpen] = useState(false);
  const [contextRow, setContextRow] = useState<NetConn | null>(null);
  const [contextPos, setContextPos] = useState<{ x: number; y: number } | null>(null);

  // Background mode state
  const [isBackground, setIsBackground] = useState(false);
  const [bgTelemetry, setBgTelemetry] = useState<BackgroundTelemetry | null>(null);

  // Check background mode on mount and periodically
  useEffect(() => {
    const check = async () => {
      try {
        const bg = await invoke<boolean>("cmd_monitor_is_background");
        setIsBackground(bg);
        if (bg) {
          const tel = await invoke<BackgroundTelemetry>("cmd_monitor_get_telemetry");
          setBgTelemetry(tel);
        }
      } catch {
        // ignore
      }
    };
    check();
    const interval = setInterval(check, 10000);
    return () => clearInterval(interval);
  }, []);

  const data = useMemo(() => query.data?.items ?? [], [query.data]);

  // Keep selected in sync with data updates (e.g., cmdline enrichment)
  const selectedConn = useMemo(() => {
    if (!selected) return null;
    // Find the updated connection in data
    return data.find(
      (c) =>
        c.proto === selected.proto &&
        c.family === selected.family &&
        c.local.addr === selected.local.addr &&
        c.local.port === selected.local.port &&
        c.remote.addr === selected.remote.addr &&
        c.remote.port === selected.remote.port &&
        c.pid === selected.pid
    ) ?? selected;
  }, [data, selected]);

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
        process_cmdline: c.process_cmdline,
        first_seen: new Date(c.first_seen * 1000).toISOString(),
        last_seen: new Date(c.last_seen * 1000).toISOString(),
        is_current: c.is_current,
      })),
      [
        "proto", "family", "local_addr", "local_port", "remote_addr",
        "remote_port", "state", "pid", "process_name", "process_path",
        "process_cmdline", "first_seen", "last_seen", "is_current",
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
      {/* Background mode banner */}
      {isBackground && (
        <div className="flex items-center gap-2 px-3 py-1.5 bg-amber-500/10 border-b border-amber-500/20 text-amber-600 text-xs">
          <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
          <span>{t("log-collector.background-mode.active")}</span>
          {bgTelemetry && (
            <span className="text-amber-500/80">
              {t("log-collector.background-mode.written")}: {bgTelemetry.events_written}
              {bgTelemetry.events_dropped > 0 && (
                <span className="text-red-500 ml-2">
                  {t("log-collector.background-mode.dropped")}: {bgTelemetry.events_dropped}
                </span>
              )}
              {bgTelemetry.last_event_at && (
                <span className="ml-2">
                  {t("log-collector.background-mode.last-event")}: {new Date(bgTelemetry.last_event_at).toLocaleTimeString()}
                </span>
              )}
            </span>
          )}
        </div>
      )}

      <NetworkToolbar
        onExport={handleExport}
        onClearHistory={() => clearMutation.mutate()}
        onRefresh={() => query.refetch()}
        onKillSelected={() => {
          if (selectedConn) setKillDialogOpen(true);
        }}
        hasSelection={selectedConn != null}
        loading={query.isFetching}
      />

      <div className="flex-1 min-h-0">
        <Group orientation={detailPosition === "bottom" ? "vertical" : "horizontal"}>
          <Panel defaultSize={detailPosition === "bottom" ? 60 : 70} minSize={40}>
            <NetworkTable
              data={data}
              onRowSelect={setSelected}
              onRowContextMenu={handleContextMenu}
              selectedRowId={selectedConn ? `${selectedConn.proto}|${selectedConn.family}|${selectedConn.local.addr}:${selectedConn.local.port}|${selectedConn.remote.addr}:${selectedConn.remote.port}|${selectedConn.pid}` : null}
            />
          </Panel>
          {selectedConn != null && (
            <>
              <Separator className={detailPosition === "bottom" ? "h-px" : "w-px"} style={{ backgroundColor: "var(--border)" }} />
              <Panel defaultSize={detailPosition === "bottom" ? 40 : 30} minSize={20}>
                <NetworkDetail conn={selectedConn} onClose={() => setSelected(null)} />
              </Panel>
            </>
          )}
        </Group>
      </div>

      <NetworkStatsBar data={data} />

      <KillProcessDialog
        conn={selectedConn}
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
