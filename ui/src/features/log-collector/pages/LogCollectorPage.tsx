import { useState, useCallback, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Panel, Group, Separator } from "react-resizable-panels";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { invoke } from "@tauri-apps/api/core";
import { useSysmonStatus, useDefaultEventConfigs, useStartCollection, useStopCollection, useUninstallSysmon, useLogMaxSize, useSyncCollectingState } from "../hooks";
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
import { AlertTriangle } from "lucide-react";

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
  const { events, collecting, selectedEvent, clearEvents, addEvents, enabledEventKeys, setEnabledEventKeys, setSelectedEvent, loadLimit, setLoadLimit } = useLogCollectorStore();
  const detailPosition = useUIStore((s) => s.detailPositions["log-collector"] ?? "right");
  const eventIds = useMemo(() =>
    eventConfigs.filter((c) => enabledEventKeys.includes(c.key)).map((c) => c.event_id),
    [eventConfigs, enabledEventKeys]
  );
  const startMutation = useStartCollection(eventIds);
  const stopMutation = useStopCollection();

  const [installLoading, setInstallLoading] = useState(false);
  const [configDialogOpen, setConfigDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<"install" | "config">("config");
  const [uninstallConfirmOpen, setUninstallConfirmOpen] = useState(false);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [loadHistoryDialogOpen, setLoadHistoryDialogOpen] = useState(false);
  const [totalEventCount, setTotalEventCount] = useState<number | null>(null);
  const [countingEvents, setCountingEvents] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [pcapAvailable, setPcapAvailable] = useState(false);
  const [pcapConfig, setPcapConfig] = useState({ enable_sni: false, enable_dns_pcap: false, adapter_ip: null as string | null, max_duration_secs: 0 });

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

  const handleLoadHistory = useCallback(() => {
    setLoadHistoryDialogOpen(true);
    setTotalEventCount(null);
    setCountingEvents(true);
    api.getEventCount(eventIds).then((count) => {
      setTotalEventCount(count);
    }).catch(() => {
      setTotalEventCount(null);
    }).finally(() => {
      setCountingEvents(false);
    });
  }, [eventIds]);

  const doLoadHistory = useCallback(async (limit: number) => {
    setLoadingHistory(true);
    try {
      const data = await api.getExistingEvents(limit, eventIds);
      if (data && data.length > 0) {
        addEvents(data);
        toast.success(t("log-collector.history-loaded", { count: data.length }));
      } else {
        toast.info(t("log-collector.no-history"));
      }
    } catch (e) {
      toast.error(t("log-collector.load-history-failed"), { description: e instanceof Error ? e.message : "Unknown error" });
    } finally {
      setLoadingHistory(false);
      setLoadHistoryDialogOpen(false);
    }
  }, [addEvents, t, eventIds]);

  const handleClear = useCallback(() => {
    setClearConfirmOpen(true);
  }, []);

  const handleExport = useCallback(async () => {
    const rows = events.map((e) => ({
      timestamp_utc: new Date(e.timestamp_epoch * 1000).toISOString(),
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
      "timestamp_utc", "event_type", "process_id", "process_name", "process_path",
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
                  {t("log-collector.background-mode.last-event")}: {formatEventTimestamp(bgTelemetry.last_event_at)}
                </span>
              )}
            </span>
          )}
        </div>
      )}

      <LogCollectorToolbar
        onStart={handleStart}
        onStop={handleStop}
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

      <Dialog open={loadHistoryDialogOpen} onOpenChange={setLoadHistoryDialogOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("log-collector.load-history-dialog.title")}</DialogTitle>
            <DialogDescription>
              {countingEvents
                ? t("log-collector.load-history-dialog.counting")
                : totalEventCount !== null
                  ? t("log-collector.load-history-dialog.description", { count: totalEventCount })
                  : t("log-collector.load-history-dialog.description-unknown")}
            </DialogDescription>
          </DialogHeader>
          <div className="flex items-center gap-2 py-2">
            <Label className="text-xs shrink-0">{t("log-collector.load-history-dialog.limit")}</Label>
            <Input
              type="number"
              min={100}
              max={100000}
              value={loadLimit}
              onChange={(e) => setLoadLimit(Math.max(100, parseInt(e.target.value) || 5000))}
              className="w-24 h-7 text-xs text-center"
            />
            <span className="text-xs text-fg-tertiary">{t("log-collector.load-history-dialog.records")}</span>
          </div>
          <DialogFooter className="flex-row gap-2 sm:justify-between">
            {totalEventCount !== null && (
              <Button variant="secondary" size="sm" onClick={() => doLoadHistory(totalEventCount)} disabled={loadingHistory || countingEvents}>
                {loadingHistory ? t("log-collector.load-history-dialog.loading") : t("log-collector.load-history-dialog.load-all")}
              </Button>
            )}
            <div className="flex-1" />
            <Button variant="secondary" size="sm" onClick={() => setLoadHistoryDialogOpen(false)}>{t("common.cancel")}</Button>
            <Button variant="default" size="sm" onClick={() => doLoadHistory(loadLimit)} disabled={loadingHistory || countingEvents}>
              {loadingHistory ? t("log-collector.load-history-dialog.loading") : t("log-collector.load-history-dialog.load")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
