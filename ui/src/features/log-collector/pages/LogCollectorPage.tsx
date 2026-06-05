import { useState, useCallback, useMemo } from "react";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable";
import { useSysmonStatus, useDefaultEventConfigs, useStartCollection, useStopCollection, useLoadHistory, useSysmonEventListener } from "../hooks";
import { useLogCollectorStore } from "../store";
import { SysmonStatusBanner } from "../components/SysmonStatusBanner";
import { SysmonInstallDialog } from "../components/SysmonInstallDialog";
import { LogCollectorToolbar } from "../components/LogCollectorToolbar";
import { EventTable } from "../components/EventTable";
import { EventDetail } from "../components/EventDetail";
import { LogCollectorStatsBar } from "../components/LogCollectorStatsBar";
import * as api from "../api";

export default function LogCollectorPage() {
  const { data: status, refetch: refetchStatus } = useSysmonStatus();
  const { data: eventConfigs = [] } = useDefaultEventConfigs();
  const startMutation = useStartCollection();
  const stopMutation = useStopCollection();
  const loadHistoryMutation = useLoadHistory();
  const { events, collecting, selectedRecordId, addEvents, clearEvents } = useLogCollectorStore();

  const [installDialogOpen, setInstallDialogOpen] = useState(false);
  const [installLoading, setInstallLoading] = useState(false);

  // Listen for real-time events
  useSysmonEventListener();

  const selectedEvent = useMemo(() => {
    if (selectedRecordId === null) return null;
    return events.find((e) => e.record_id === selectedRecordId) ?? null;
  }, [events, selectedRecordId]);

  const handleInstall = useCallback(async (acceptEula: boolean, enabledEvents: string[]) => {
    setInstallLoading(true);
    try {
      // Generate config with selected events
      await api.generateConfig(enabledEvents);
      const [ok, msg] = await api.install(acceptEula);
      if (!ok) {
        // Could show a toast here
        console.error("Install failed:", msg);
      }
      refetchStatus();
    } catch (e) {
      console.error("Install error:", e);
    } finally {
      setInstallLoading(false);
      setInstallDialogOpen(false);
    }
  }, [refetchStatus]);

  const handleStart = useCallback(async () => {
    if (!status?.running) {
      setInstallDialogOpen(true);
      return;
    }
    try {
      await startMutation.mutateAsync();
    } catch (e) {
      console.error("Start error:", e);
    }
  }, [status, startMutation]);

  const handleStop = useCallback(async () => {
    try {
      await stopMutation.mutateAsync();
    } catch (e) {
      console.error("Stop error:", e);
    }
  }, [stopMutation]);

  const handleLoadHistory = useCallback(async () => {
    try {
      const history = await loadHistoryMutation.mutateAsync(5000);
      addEvents(history);
    } catch (e) {
      console.error("Load history error:", e);
    }
  }, [loadHistoryMutation, addEvents]);

  const handleClear = useCallback(() => {
    clearEvents();
  }, [clearEvents]);

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

  return (
    <div className="flex flex-col h-full">
      <SysmonStatusBanner status={status ?? null} onInstall={() => setInstallDialogOpen(true)} />

      <LogCollectorToolbar
        onStart={handleStart}
        onStop={handleStop}
        onLoadHistory={handleLoadHistory}
        onClear={handleClear}
        onExport={handleExport}
        collecting={collecting}
        loading={startMutation.isPending || stopMutation.isPending || loadHistoryMutation.isPending}
      />

      <ResizablePanelGroup direction="horizontal" className="flex-1">
        <ResizablePanel defaultSize={65} minSize={40}>
          <EventTable events={events} />
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel defaultSize={35} minSize={25}>
          <EventDetail event={selectedEvent} />
        </ResizablePanel>
      </ResizablePanelGroup>

      <LogCollectorStatsBar sysmonRunning={status?.running ?? false} />

      <SysmonInstallDialog
        open={installDialogOpen}
        onOpenChange={setInstallDialogOpen}
        eventConfigs={eventConfigs}
        onInstall={handleInstall}
        loading={installLoading}
      />
    </div>
  );
}
