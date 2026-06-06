import { useState, useCallback, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { useSysmonStatus, useDefaultEventConfigs, useStartCollection, useStopCollection, useLoadHistory, useUninstallSysmon, useLogMaxSize, useSyncCollectingState } from "../hooks";
import { useLogCollectorStore } from "../store";
import { SysmonInstallDialog } from "../components/SysmonInstallDialog";
import { LogCollectorToolbar } from "../components/LogCollectorToolbar";
import { EventTable } from "../components/EventTable";
import { EventDetail } from "../components/EventDetail";
import { LogCollectorStatsBar } from "../components/LogCollectorStatsBar";
import { LogCollectorConfigDialog } from "../components/LogCollectorConfigDialog";
import { toast } from "sonner";
import * as api from "../api";

export default function LogCollectorPage() {
  const { t } = useTranslation();
  const { data: status, refetch: refetchStatus } = useSysmonStatus();
  const { data: eventConfigs = [] } = useDefaultEventConfigs();
  const { data: logMaxSizeMb = 0 } = useLogMaxSize();
  const uninstallMutation = useUninstallSysmon();
  const { events, collecting, selectedEvent, addEvents, clearEvents, enabledEventKeys, setEnabledEventKeys } = useLogCollectorStore();
  const eventIds = useMemo(() =>
    eventConfigs.filter((c) => enabledEventKeys.includes(c.key)).map((c) => c.event_id),
    [eventConfigs, enabledEventKeys]
  );
  const startMutation = useStartCollection(eventIds);
  const stopMutation = useStopCollection();
  const loadHistoryMutation = useLoadHistory(eventIds);

  const [installDialogOpen, setInstallDialogOpen] = useState(false);
  const [installLoading, setInstallLoading] = useState(false);
  const [configDialogOpen, setConfigDialogOpen] = useState(false);
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

  const handleInstall = useCallback(async (acceptEula: boolean, enabledEvents: string[]) => {
    setInstallLoading(true);
    try {
      await api.generateConfig(enabledEvents);
      const [ok, msg] = await api.install(acceptEula);
      if (!ok) {
        toast.error("安装失败", { description: msg });
      }
      refetchStatus();
    } catch (e) {
      toast.error("安装异常", { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setInstallLoading(false);
      setInstallDialogOpen(false);
    }
  }, [refetchStatus]);

  const handleStart = useCallback(async () => {
    if (!status?.installed || !status?.running) {
      setInstallDialogOpen(true);
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
    setConfigDialogOpen(true);
  }, []);

  const handleApplyConfig = useCallback(async (enabledEvents: string[], logSizeMb: number, newPcapConfig: { enable_sni: boolean; enable_dns_pcap: boolean }) => {
    setInstallLoading(true);
    try {
      await api.generateConfig(enabledEvents);
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
      } catch (e) {
        console.warn("pcap config update failed:", e);
      }

      // If currently collecting, restart subscription with new event IDs
      if (collecting) {
        await api.stopSubscription();
        const eventIds = eventConfigs
          .filter((c) => enabledEvents.includes(c.key))
          .map((c) => c.event_id);
        await api.startSubscription(eventIds.length > 0 ? eventIds : [3008, 22, 3], 500);
      }
      refetchStatus();
    } catch (e) {
      toast.error("配置应用失败", { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setInstallLoading(false);
      setConfigDialogOpen(false);
    }
  }, [refetchStatus, collecting, eventConfigs, setEnabledEventKeys]);

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

      <ResizablePanelGroup direction="horizontal" className="flex-1">
        <ResizablePanel defaultSize={65} minSize={40}>
          <EventTable events={events} />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={35} minSize={25}>
          <EventDetail event={selectedEventDetail} />
        </ResizablePanel>
      </ResizablePanelGroup>

      <LogCollectorStatsBar sysmonRunning={status?.running ?? false} logMaxSizeMb={logMaxSizeMb} />

      <SysmonInstallDialog
        open={installDialogOpen}
        onOpenChange={setInstallDialogOpen}
        eventConfigs={eventConfigs}
        onInstall={handleInstall}
        loading={installLoading}
      />

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
