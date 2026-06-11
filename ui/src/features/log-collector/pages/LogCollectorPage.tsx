import { useState, useCallback, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Panel, Group, Separator } from "react-resizable-panels";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { useSysmonStatus, useDefaultEventConfigs, useStartCollection, useStopCollection, useLoadHistory, useUninstallSysmon, useLogMaxSize, useSyncCollectingState } from "../hooks";
import { useLogCollectorStore } from "../store";
import { useUIStore } from "@/stores/ui-store";
import { LogCollectorToolbar } from "../components/LogCollectorToolbar";
import { EventTable } from "../components/EventTable";
import { EventDetail } from "../components/EventDetail";
import { LogCollectorStatsBar } from "../components/LogCollectorStatsBar";
import { LogCollectorConfigDialog } from "../components/LogCollectorConfigDialog";
import { toast } from "sonner";
import * as api from "../api";

/**
 * 将 MonitorEvent 转换为 SysmonEvent 格式，从 raw_json 解析出所有字段
 * raw_json 存储的是完整的 SysmonEvent 或 PcapEvent 的 JSON 序列化
 */
export function monitorEventToSysmonEvent(me: api.MonitorEvent): any {
  let raw: any = {};
  try {
    raw = JSON.parse(me.raw_json);
  } catch {}

  const isPcap = me.source === "pcap";
  const isNetMonitor = me.source === "net_monitor";

  if (isPcap) {
    const eventKind = raw.event_kind === "tls_sni" ? "tls_sni" : "dns_pcap";
    return {
      event_id: 0,
      event_type: eventKind,
      timestamp: new Date(me.timestamp).toISOString(),
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
      timestamp: new Date(me.timestamp).toISOString(),
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
    timestamp: raw.timestamp || new Date(me.timestamp).toISOString(),
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

export default function LogCollectorPage() {
  const { t } = useTranslation();
  const { data: status, refetch: refetchStatus } = useSysmonStatus();
  const { data: eventConfigs = [] } = useDefaultEventConfigs();
  const { data: logMaxSizeMb = 0 } = useLogMaxSize();
  const uninstallMutation = useUninstallSysmon();
  const { events, collecting, selectedEvent, addEvents, clearEvents, enabledEventKeys, setEnabledEventKeys, setSelectedEvent } = useLogCollectorStore();
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
  const [pcapConfig, setPcapConfig] = useState({ enable_sni: true, enable_dns_pcap: true });

  useEffect(() => {
    invoke<boolean>("cmd_pcap_is_available").then(setPcapAvailable).catch(() => setPcapAvailable(false));
    api.monitorGetConfig().then((c) => {
      setPcapConfig({ enable_sni: c.enable_sni, enable_dns_pcap: c.enable_dns_pcap });
    }).catch(() => {});
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
      toast.error("启动失败", { description: e instanceof Error ? e.message : "未知错误" });
    }
  }, [status, startMutation]);

  const handleStop = useCallback(async () => {
    try {
      await stopMutation.mutateAsync();
    } catch (e) {
      toast.error("停止失败", { description: e instanceof Error ? e.message : "未知错误" });
    }
  }, [stopMutation]);

  const handleLoadHistory = useCallback(async () => {
    try {
      const history = await loadHistoryMutation.mutateAsync(5000);
      addEvents(history);
    } catch (e) {
      toast.error("加载历史失败", { description: e instanceof Error ? e.message : "未知错误" });
    }
  }, [loadHistoryMutation, addEvents]);

  const handleClear = useCallback(() => {
    setClearConfirmOpen(true);
  }, []);

  const handleExport = useCallback(() => {
    const json = JSON.stringify(events, null, 2);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `sysmon-events-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [events]);

  const handleUninstall = useCallback(() => {
    setUninstallConfirmOpen(true);
  }, []);

  const doUninstall = useCallback(async () => {
    try {
      const [ok, msg] = await uninstallMutation.mutateAsync();
      if (!ok) {
        toast.error("卸载失败", { description: msg });
      }
      refetchStatus();
    } catch (e) {
      toast.error("卸载异常", { description: e instanceof Error ? e.message : "未知错误" });
    }
  }, [uninstallMutation, refetchStatus]);

  const handleOpenConfigDialog = useCallback(() => {
    setDialogMode("config");
    setConfigDialogOpen(true);
  }, []);

  const handleApplyConfig = useCallback(async (enabledEvents: string[], logSizeMb: number, newPcapConfig: { enable_sni: boolean; enable_dns_pcap: boolean }) => {
    setInstallLoading(true);
    try {
      await api.generateConfig(enabledEvents);

      if (dialogMode === "install") {
        // Install sysmon
        const [ok, msg] = await api.install(true);
        if (!ok) {
          toast.error("安装失败", { description: msg });
          return;
        }
        // Auto-start collection after successful install
        await refetchStatus();
        try {
          await startMutation.mutateAsync();
        } catch (e) {
          toast.error("启动采集失败", { description: e instanceof Error ? e.message : "未知错误" });
        }
      } else {
        // Update config (existing behavior)
        const [ok, msg] = await api.updateConfig();
        if (!ok) {
          toast.error("配置更新失败", { description: msg });
        }
        await api.setLogMaxSize(logSizeMb);
        setEnabledEventKeys(enabledEvents);

        // Update pcap config
        try {
          const config = await api.monitorGetConfig();
          config.enable_sni = newPcapConfig.enable_sni;
          config.enable_dns_pcap = newPcapConfig.enable_dns_pcap;
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
      toast.error(dialogMode === "install" ? "安装异常" : "配置应用失败", { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setInstallLoading(false);
      setConfigDialogOpen(false);
    }
  }, [refetchStatus, dialogMode, startMutation, collecting, eventConfigs, setEnabledEventKeys]);

  return (
    <div className="flex flex-col h-full">
      <LogCollectorToolbar
        onStart={handleStart}
        onStop={handleStop}
        onLoadHistory={handleLoadHistory}
        onClear={handleClear}
        onExport={handleExport}
        onUninstall={handleUninstall}
        onOpenConfigDialog={handleOpenConfigDialog}
        collecting={collecting}
        loading={startMutation.isPending || stopMutation.isPending || loadHistoryMutation.isPending || uninstallMutation.isPending}
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
