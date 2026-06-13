import { useState, useCallback, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Panel, Group, Separator } from "react-resizable-panels";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { invoke } from "@tauri-apps/api/core";
import { useSysmonStatus, useDefaultEventConfigs, useStartCollection, useStopCollection, useLoadHistory, useSearchEventPage, useUninstallSysmon, useLogMaxSize, useSyncCollectingState } from "../hooks";
import { useLogCollectorStore } from "../store";
import { useUIStore } from "@/stores/ui-store";
import { LogCollectorToolbar } from "../components/LogCollectorToolbar";
import { EventTable } from "../components/EventTable";
import { EventDetail } from "../components/EventDetail";
import { LogCollectorStatsBar } from "../components/LogCollectorStatsBar";
import { LogCollectorConfigDialog } from "../components/LogCollectorConfigDialog";
import { toast } from "sonner";
import * as api from "../api";
import type { MonitorEvent } from "../types";
import { formatEventTimestamp } from "@/lib/utils";
import { exportCsv } from "@/lib/csv";
import { EVENT_TYPE_LABELS, EVENT_TYPE_COLORS } from "../types";
import type { ExtendedSysmonEventType } from "../types";
import { DataTable } from "@/components/data-table/DataTable";
import { type ColumnDef } from "@tanstack/react-table";
import { AlertTriangle, Database, ChevronLeft, ChevronRight } from "lucide-react";

/**
 * 将 MonitorEvent 转换为 SysmonEvent 格式，从 raw_json 解析出所有字段
 * raw_json 存储的是完整的 SysmonEvent 或 PcapEvent 的 JSON 序列化
 */
export function monitorEventToSysmonEvent(me: MonitorEvent): any {
  let raw: any = {};
  try {
    raw = JSON.parse(me.raw_json);
  } catch {}

  const isPcap = me.source === "Pcap";
  const isNetMonitor = me.source === "NetMonitor";

  if (isPcap) {
    const eventKind = raw.event_kind === "tls_sni" ? "tls_sni" : "dns_pcap";
    return {
      event_id: 0,
      event_type: eventKind,
      timestamp: formatEventTimestamp(me.timestamp),
      timestamp_epoch: me.timestamp / 1000,
      timestamp_valid: true,
      record_id: me.id,
      raw_data: {},
      process_id: 0,
      process_name: me.process_name || "",
      process_path: "",
      user: "",
      rule_name: "",
      query_name: raw.domain || me.key_field,
      query_results: raw.query_type || "",
      query_status: 0,
      source_ip: raw.src_ip || "",
      source_port: raw.src_port || 0,
      destination_ip: raw.dst_ip || "",
      destination_port: raw.dst_port || 0,
      protocol: eventKind === "tls_sni" ? "TCP" : "UDP",
      initiated: true,
      is_external: false,
      source_process_id: 0,
      source_process_name: "",
      source_process_path: "",
      target_process_id: 0,
      target_process_name: "",
      target_process_path: "",
      start_address: "",
      start_module: "",
      start_function: "",
      is_suspicious: false,
      target_filename: "",
      creation_utc_time: "",
      _rawJson: me.raw_json,
      _source: me.source,
    };
  }

  if (isNetMonitor) {
    return {
      event_id: 0,
      event_type: "network_monitor",
      timestamp: formatEventTimestamp(me.timestamp),
      timestamp_epoch: me.timestamp / 1000,
      timestamp_valid: true,
      record_id: me.id,
      raw_data: {},
      process_id: raw.pid || 0,
      process_name: raw.process_name || me.process_name,
      process_path: raw.process_path || "",
      user: "",
      rule_name: "",
      query_name: "",
      query_results: "",
      query_status: 0,
      source_ip: raw.local?.addr || "",
      source_port: raw.local?.port || 0,
      destination_ip: raw.remote?.addr || "",
      destination_port: raw.remote?.port || 0,
      protocol: raw.proto ? String(raw.proto).toUpperCase() : "",
      initiated: true,
      is_external: false,
      source_process_id: 0,
      source_process_name: "",
      source_process_path: "",
      target_process_id: 0,
      target_process_name: "",
      target_process_path: "",
      start_address: "",
      start_module: "",
      start_function: "",
      is_suspicious: false,
      target_filename: "",
      creation_utc_time: "",
      _rawJson: me.raw_json,
      _source: me.source,
      _state: raw.state || "",
    };
  }

  // SysmonEvent: 直接从 raw 中提取所有字段
  return {
    event_id: raw.event_id || 0,
    event_type: me.event_type,
    timestamp: (typeof raw.timestamp === 'number') ? formatEventTimestamp(raw.timestamp) : (raw.timestamp || formatEventTimestamp(me.timestamp)),
    timestamp_epoch: raw.timestamp_epoch || me.timestamp / 1000,
    timestamp_valid: raw.timestamp_valid ?? true,
    record_id: me.id,
    raw_data: raw.raw_data || {},
    process_id: raw.process_id || 0,
    process_name: raw.process_name || me.process_name,
    process_path: raw.process_path || "",
    user: raw.user || "",
    rule_name: raw.rule_name || "",
    query_name: raw.query_name || me.key_field,
    query_results: raw.query_results || "",
    query_status: raw.query_status || 0,
    source_ip: raw.source_ip || "",
    source_port: raw.source_port || 0,
    destination_ip: raw.destination_ip || "",
    destination_port: raw.destination_port || 0,
    protocol: raw.protocol || "",
    initiated: raw.initiated ?? true,
    is_external: raw.is_external ?? false,
    source_process_id: raw.source_process_id || 0,
    source_process_name: raw.source_process_name || "",
    source_process_path: raw.source_process_path || "",
    target_process_id: raw.target_process_id || 0,
    target_process_name: raw.target_process_name || "",
    target_process_path: raw.target_process_path || "",
    start_address: raw.start_address || "",
    start_module: raw.start_module || "",
    start_function: raw.start_function || "",
    is_suspicious: raw.is_suspicious ?? false,
    target_filename: raw.target_filename || "",
    creation_utc_time: raw.creation_utc_time || "",
    _rawJson: me.raw_json,
    _source: me.source,
  };
}

// --- History query panel ---
type HistorySource = "all" | "Sysmon" | "DnsClient" | "Pcap" | "NetMonitor";

function getHistoryDestination(event: any): string {
  const et = event.event_type as ExtendedSysmonEventType;
  switch (et) {
    case "network_connect":
      return `${event.destination_ip}:${event.destination_port}`;
    case "dns":
    case "dns_client":
    case "tls_sni":
    case "dns_pcap":
      return event.query_name || "-";
    case "create_remote_thread":
      return `${event.source_process_name} → ${event.target_process_name}`;
    case "file_create":
      return event.target_filename || "-";
    default:
      return event.process_name || "-";
  }
}

function HistoryQueryPanel({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const searchEventPage = useSearchEventPage();
  const [source, setSource] = useState<HistorySource>("all");
  const [eventType, setEventType] = useState<string>("all");
  const [processName, setProcessName] = useState("");
  const [keyField, setKeyField] = useState("");
  const [searchText, setSearchText] = useState("");
  const [historyEvents, setHistoryEvents] = useState<any[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [currentOffset, setCurrentOffset] = useState(0);
  const [pageSize] = useState(200);
  const [selectedHistoryEvent, setSelectedHistoryEvent] = useState<any>(null);

  const handleSearch = useCallback(async (offset = 0) => {
    try {
      const result = await searchEventPage.mutateAsync({
        source: source === "all" ? null : source,
        event_type: eventType === "all" ? null : eventType,
        process_name: processName || null,
        key_field: keyField || null,
        search_text: searchText || null,
        limit: pageSize,
        offset,
      });
      const converted = result.items.map(monitorEventToSysmonEvent);
      setHistoryEvents(offset === 0 ? converted : (prev) => [...prev, ...converted]);
      setTotalCount(result.total);
      setCurrentOffset(offset + result.items.length);
    } catch (e) {
      toast.error(t("log-collector.history.search-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
    }
  }, [source, eventType, processName, keyField, searchText, pageSize, searchEventPage, t]);

  const handleLoadMore = useCallback(() => {
    handleSearch(currentOffset);
  }, [handleSearch, currentOffset]);

  const columns = useMemo<ColumnDef<any, unknown>[]>(() => [
    {
      accessorKey: "timestamp",
      header: t("log-collector.table.time"),
      size: 144,
      cell: ({ getValue }) => <span className="font-mono text-fg-secondary whitespace-nowrap overflow-hidden text-ellipsis">{getValue() as string || "-"}</span>,
    },
    {
      accessorKey: "event_type",
      header: t("log-collector.table.type"),
      size: 96,
      cell: ({ getValue }) => {
        const et = getValue() as string;
        return <span className={`inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap ${EVENT_TYPE_COLORS[et as ExtendedSysmonEventType] || ""}`}>
          {EVENT_TYPE_LABELS[et as ExtendedSysmonEventType] || et}
        </span>;
      },
    },
    {
      id: "destination",
      accessorFn: (row) => getHistoryDestination(row),
      header: t("log-collector.table.destination"),
      size: 176,
      cell: ({ row }) => <span className="truncate text-fg-primary" title={getHistoryDestination(row.original)}>{getHistoryDestination(row.original)}</span>,
    },
    {
      accessorKey: "process_path",
      header: t("log-collector.table.path"),
      size: 300,
      cell: ({ getValue }) => <span className="truncate text-fg-secondary" title={getValue() as string}>{getValue() as string}</span>,
    },
  ], [t]);

  return (
    <div className="flex flex-col h-full">
      {/* Search controls */}
      <div className="flex flex-wrap items-end gap-2 p-2 border-b border-border bg-bg-elev-1">
        <div className="flex flex-col gap-0.5">
          <Label className="text-[10px]">{t("log-collector.history.source")}</Label>
          <Select value={source} onValueChange={(v: HistorySource) => setSource(v)}>
            <SelectTrigger className="h-7 w-28 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("log-collector.history.all")}</SelectItem>
              <SelectItem value="Sysmon">Sysmon</SelectItem>
              <SelectItem value="DnsClient">DNS Client</SelectItem>
              <SelectItem value="Pcap">Pcap</SelectItem>
              <SelectItem value="NetMonitor">{t("log-collector.history.net-monitor")}</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-0.5">
          <Label className="text-[10px]">{t("log-collector.history.event-type")}</Label>
          <Select value={eventType} onValueChange={setEventType}>
            <SelectTrigger className="h-7 w-28 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("log-collector.history.all")}</SelectItem>
              <SelectItem value="dns">DNS</SelectItem>
              <SelectItem value="dns_client">DNS Client</SelectItem>
              <SelectItem value="network_connect">{t("log-collector.history.network-connect")}</SelectItem>
              <SelectItem value="network_monitor">{t("log-collector.history.net-monitor")}</SelectItem>
              <SelectItem value="tls_sni">TLS SNI</SelectItem>
              <SelectItem value="dns_pcap">DNS Pcap</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-0.5">
          <Label className="text-[10px]">{t("log-collector.history.process-name")}</Label>
          <Input type="text" value={processName} onChange={(e) => setProcessName(e.target.value)} placeholder={t("log-collector.history.fuzzy-match")} className="h-7 w-28 text-xs" />
        </div>
        <div className="flex flex-col gap-0.5">
          <Label className="text-[10px]">IP/{t("log-collector.history.domain")}</Label>
          <Input type="text" value={keyField} onChange={(e) => setKeyField(e.target.value)} placeholder={t("log-collector.history.fuzzy-match")} className="h-7 w-28 text-xs" />
        </div>
        <div className="flex flex-col gap-0.5">
          <Label className="text-[10px]">{t("log-collector.history.fulltext")}</Label>
          <Input type="text" value={searchText} onChange={(e) => setSearchText(e.target.value)} placeholder="raw_json" className="h-7 w-36 text-xs" />
        </div>
        <div className="flex items-end gap-2">
          <Button variant="default" size="sm" onClick={() => handleSearch(0)} disabled={searchEventPage.isPending} className="h-7 text-xs">
            {searchEventPage.isPending ? t("log-collector.history.searching") : t("log-collector.history.search")}
          </Button>
          {currentOffset < totalCount && historyEvents.length > 0 && (
            <Button variant="secondary" size="sm" onClick={handleLoadMore} disabled={searchEventPage.isPending} className="h-7 text-xs">
              <ChevronRight className="h-3 w-3 mr-0.5" />
              {t("log-collector.history.load-more")}
            </Button>
          )}
          <div className="flex-1" />
          <Button variant="ghost" size="sm" onClick={onClose} className="h-7 text-xs">
            <ChevronLeft className="h-3 w-3 mr-0.5" />
            {t("log-collector.history.back-to-live")}
          </Button>
        </div>
      </div>

      {/* Results */}
      <div className="flex-1 min-h-0">
        <Group orientation="horizontal">
          <Panel defaultSize={selectedHistoryEvent ? 70 : 100} minSize={40}>
            <DataTable
              columns={columns}
              data={historyEvents}
              getRowId={(e) => `${e.record_id}-${e.timestamp}`}
              onRowSelect={(row) => row && setSelectedHistoryEvent(row)}
              selectedRowId={selectedHistoryEvent ? `${selectedHistoryEvent.record_id}-${selectedHistoryEvent.timestamp}` : null}
              empty={t("log-collector.history.no-results")}
              persistKey="log-collector-history"
            />
          </Panel>
          {selectedHistoryEvent != null && (
            <>
              <Separator className="w-px bg-border hover:bg-accent transition-colors" />
              <Panel defaultSize={30} minSize={20}>
                <EventDetail event={selectedHistoryEvent} onClose={() => setSelectedHistoryEvent(null)} />
              </Panel>
            </>
          )}
        </Group>
      </div>

      {/* Stats bar */}
      <div className="flex items-center gap-3 px-3 py-1.5 border-t border-border bg-bg-elev-1 text-[10px] text-fg-tertiary">
        <span>{t("log-collector.history.total")}: {totalCount}</span>
        <span>{t("log-collector.history.loaded")}: {historyEvents.length}</span>
      </div>
    </div>
  );
}

// --- Background mode telemetry ---
interface BackgroundTelemetry {
  events_written: number;
  events_dropped: number;
  last_event_at: number | null;
}

export default function LogCollectorPage() {
  const { t } = useTranslation();
  const { data: status, refetch: refetchStatus } = useSysmonStatus();
  const { data: eventConfigs = [] } = useDefaultEventConfigs();
  const { data: logMaxSizeMb = 0 } = useLogMaxSize();
  const uninstallMutation = useUninstallSysmon();
  const { events, collecting, selectedEvent, clearEvents, addEvents, enabledEventKeys, setEnabledEventKeys, setSelectedEvent } = useLogCollectorStore();
  const detailPosition = useUIStore((s) => s.detailPositions["log-collector"] ?? "right");
  const eventIds = useMemo(() =>
    eventConfigs.filter((c) => enabledEventKeys.includes(c.key)).map((c) => c.event_id),
    [eventConfigs, enabledEventKeys]
  );
  const startMutation = useStartCollection(eventIds);
  const stopMutation = useStopCollection();
  const loadHistoryMutation = useLoadHistory(eventIds);

  const [installLoading, setInstallLoading] = useState(false);
  const [configDialogOpen, setConfigDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"install" | "config">("config");
  const [uninstallConfirmOpen, setUninstallConfirmOpen] = useState(false);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [pcapAvailable, setPcapAvailable] = useState(false);
  const [pcapConfig, setPcapConfig] = useState({ enable_sni: false, enable_dns_pcap: false, adapter_ip: null as string | null, max_duration_secs: 0 });

  // History query panel state
  const [showHistory, setShowHistory] = useState(false);

  // Background mode state
  const [isBackground, setIsBackground] = useState(false);
  const [bgTelemetry, setBgTelemetry] = useState<BackgroundTelemetry | null>(null);

  useEffect(() => {
    invoke<boolean>("cmd_pcap_is_available").then(setPcapAvailable).catch(() => setPcapAvailable(false));
    api.monitorGetConfig().then((c) => {
      setPcapConfig({ enable_sni: c.enable_sni, enable_dns_pcap: c.enable_dns_pcap, adapter_ip: c.adapter_ip ?? null, max_duration_secs: c.max_duration_secs ?? 0 });
    }).catch(() => {});
  }, []);

  // Check background mode on mount and periodically
  useEffect(() => {
    const check = async () => {
      try {
        const bg = await api.monitorIsBackground();
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

  // Sync collecting state with backend on mount (e.g. after page refresh)
  useSyncCollectingState();

  const selectedEventDetail = selectedEvent;

  const handleStart = useCallback(async () => {
    if (!status?.installed || !status?.running) {
      setDialogMode("install");
      setConfigDialogOpen(true);
      return;
    }
    try {
      await startMutation.mutateAsync();
    } catch (e) {
      toast.error(t("log-collector.start-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
    }
  }, [status, startMutation, t]);

  const handleStop = useCallback(async () => {
    try {
      await stopMutation.mutateAsync();
    } catch (e) {
      toast.error(t("log-collector.stop-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
    }
  }, [stopMutation, t]);

  const handleOpenHistory = useCallback(() => {
    setShowHistory(true);
  }, []);

  const handleLoadHistory = useCallback(() => {
    loadHistoryMutation.mutate(5000, {
      onSuccess: (data) => {
        if (data && data.length > 0) {
          addEvents(data);
          toast.success(t("log-collector.history-loaded", { count: data.length }));
        } else {
          toast.info(t("log-collector.no-history"));
        }
      },
      onError: (e) => {
        toast.error(t("log-collector.load-history-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
      },
    });
  }, [loadHistoryMutation, addEvents, t]);

  const handleClear = useCallback(() => {
    setClearConfirmOpen(true);
  }, []);

  const handleExport = useCallback(async () => {
    const rows = events.map((e) => ({
      timestamp: e.timestamp,
      event_type: e.event_type,
      process_id: e.process_id,
      process_name: e.process_name,
      process_path: e.process_path,
      user: e.user,
      source_ip: e.source_ip,
      source_port: e.source_port,
      destination_ip: e.destination_ip,
      destination_port: e.destination_port,
      protocol: e.protocol,
      initiated: e.initiated ? "是" : "否",
      is_external: e.is_external ? "是" : "否",
      query_name: e.query_name,
      query_results: e.query_results,
      source_process_name: e.source_process_name,
      source_process_path: e.source_process_path,
      target_process_name: e.target_process_name,
      target_process_path: e.target_process_path,
      start_address: e.start_address,
      target_filename: e.target_filename,
    }));
    await exportCsv(rows, [
      "timestamp", "event_type", "process_id", "process_name", "process_path",
      "user", "source_ip", "source_port", "destination_ip", "destination_port",
      "protocol", "initiated", "is_external", "query_name", "query_results",
      "source_process_name", "source_process_path", "target_process_name",
      "target_process_path", "start_address", "target_filename",
    ], `sysmon-events-${Date.now()}.csv`);
  }, [events]);

  const handleUninstall = useCallback(() => {
    setUninstallConfirmOpen(true);
  }, []);

  const doUninstall = useCallback(async () => {
    try {
      // Stop collection if running before uninstalling
      if (collecting) {
        await stopMutation.mutateAsync();
      }
      const [ok, msg] = await uninstallMutation.mutateAsync();
      if (!ok) {
        toast.error(t("log-collector.uninstall-failed"), { description: msg });
      }
      refetchStatus();
    } catch (e) {
      toast.error(t("log-collector.uninstall-error"), { description: e instanceof Error ? e.message : "Unknown error" });
    }
  }, [collecting, stopMutation, uninstallMutation, refetchStatus, t]);

  const handleOpenConfigDialog = useCallback(() => {
    setDialogMode("config");
    setConfigDialogOpen(true);
  }, []);

  const handleApplyConfig = useCallback(async (enabledEvents: string[], logSizeMb: number, newPcapConfig: { enable_sni: boolean; enable_dns_pcap: boolean; adapter_ip: string | null; max_duration_secs: number }) => {
    setInstallLoading(true);
    try {
      await api.generateConfig(enabledEvents);

      if (dialogMode === "install") {
        // Install sysmon
        const [ok, msg] = await api.install(true);
        if (!ok) {
          toast.error(t("log-collector.install-failed"), { description: msg });
          return;
        }
        // Auto-start collection after successful install
        await refetchStatus();
        try {
          await startMutation.mutateAsync();
        } catch (e) {
          toast.error(t("log-collector.start-collect-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
        }
      } else {
        // Update config (existing behavior)
        const [ok, msg] = await api.updateConfig();
        if (!ok) {
          toast.error(t("log-collector.config-update-failed"), { description: msg });
        }
        await api.setLogMaxSize(logSizeMb);
        setEnabledEventKeys(enabledEvents);

        // Update pcap config
        try {
          const config = await api.monitorGetConfig();
          config.enable_sni = newPcapConfig.enable_sni;
          config.enable_dns_pcap = newPcapConfig.enable_dns_pcap;
          config.adapter_ip = newPcapConfig.adapter_ip;
          config.max_duration_secs = newPcapConfig.max_duration_secs;
          await api.monitorUpdateConfig(config);
          setPcapConfig(newPcapConfig);

          // Start/stop pcap based on config
          if (newPcapConfig.enable_sni || newPcapConfig.enable_dns_pcap) {
            try { await invoke("cmd_pcap_stop"); } catch {}
            await invoke("cmd_pcap_start", { config: newPcapConfig });
          } else {
            try { await invoke("cmd_pcap_stop"); } catch {}
          }
        } catch {
        }

        // If currently collecting, restart subscription with new event IDs
        if (collecting) {
          await api.stopSubscription();
          const eventIds = eventConfigs
            .filter((c) => enabledEvents.includes(c.key))
            .map((c) => c.event_id);
          await api.startSubscription(eventIds.length > 0 ? eventIds : [3008, 22, 3], 500);
        }
      }
      refetchStatus();
    } catch (e) {
      toast.error(dialogMode === "install" ? t("log-collector.install-error") : t("log-collector.config-apply-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
    } finally {
      setInstallLoading(false);
      setConfigDialogOpen(false);
    }
  }, [refetchStatus, dialogMode, startMutation, collecting, eventConfigs, setEnabledEventKeys, t]);

  // If showing history panel, render it instead of live view
  if (showHistory) {
    return (
      <div className="flex flex-col h-full">
        <HistoryQueryPanel onClose={() => setShowHistory(false)} />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
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
          <div className="flex-1" />
          <Button variant="secondary" size="sm" onClick={handleOpenHistory} className="h-6 text-[10px] bg-amber-500/20 hover:bg-amber-500/30 text-amber-600 border-amber-500/30">
            <Database className="h-3 w-3 mr-1" />
            {t("log-collector.background-mode.view-history")}
          </Button>
        </div>
      )}

      <LogCollectorToolbar
        onStart={handleStart}
        onStop={handleStop}
        onOpenHistory={handleOpenHistory}
        onLoadHistory={handleLoadHistory}
        onClear={handleClear}
        onExport={handleExport}
        onUninstall={handleUninstall}
        onOpenConfigDialog={handleOpenConfigDialog}
        collecting={collecting}
        loading={startMutation.isPending || stopMutation.isPending || uninstallMutation.isPending}
        sysmonInstalled={status?.installed ?? false}
        events={events}
      />

      <div className="flex-1 min-h-0">
        <Group orientation={detailPosition === "bottom" ? "vertical" : "horizontal"}>
          <Panel defaultSize={detailPosition === "bottom" ? 60 : 70} minSize={40}>
            <EventTable events={events} />
          </Panel>
          {selectedEvent != null && (
            <>
              <Separator className={detailPosition === "bottom" ? "h-px bg-border hover:bg-accent transition-colors" : "w-px bg-border hover:bg-accent transition-colors"} />
              <Panel defaultSize={detailPosition === "bottom" ? 40 : 30} minSize={20}>
                <EventDetail event={selectedEventDetail} onClose={() => setSelectedEvent(null)} />
              </Panel>
            </>
          )}
        </Group>
      </div>

      <LogCollectorStatsBar sysmonRunning={status?.running ?? false} logMaxSizeMb={logMaxSizeMb} />

      <LogCollectorConfigDialog
        open={configDialogOpen}
        onOpenChange={setConfigDialogOpen}
        eventConfigs={eventConfigs}
        onApply={handleApplyConfig}
        loading={installLoading}
        currentLogSizeMb={logMaxSizeMb}
        currentEnabledKeys={enabledEventKeys}
        currentPcapConfig={pcapConfig}
        pcapAvailable={pcapAvailable}
        mode={dialogMode}
      />

      <Dialog open={uninstallConfirmOpen} onOpenChange={setUninstallConfirmOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("log-collector.uninstall-confirm.title")}</DialogTitle>
            <DialogDescription>{t("log-collector.uninstall-confirm.message")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" size="sm" onClick={() => setUninstallConfirmOpen(false)}>{t("common.cancel")}</Button>
            <Button variant="destructive" size="sm" onClick={async () => { setUninstallConfirmOpen(false); await doUninstall(); }}>{t("log-collector.uninstall-confirm.confirm")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={clearConfirmOpen} onOpenChange={setClearConfirmOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("log-collector.clear-confirm.title")}</DialogTitle>
            <DialogDescription>{t("log-collector.clear-confirm.message")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" size="sm" onClick={() => setClearConfirmOpen(false)}>{t("common.cancel")}</Button>
            <Button variant="destructive" size="sm" onClick={() => { setClearConfirmOpen(false); clearEvents(); }}>{t("log-collector.clear-confirm.confirm")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
