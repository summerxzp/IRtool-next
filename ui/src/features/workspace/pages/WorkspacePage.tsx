import { useEffect, useMemo, useState, useCallback } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { useWorkspaceStore } from "../store";
import { useWorkspaceAutoruns, useWorkspaceNetwork, useWorkspaceEvents, useWorkspaceRefresh, useWorkspaceSearch, useWorkspaceRuleScan } from "../hooks";
import { loadRules } from "../rules/storage";
import { networkKey, eventKey } from "../rules/engine";
import { WorkspaceToolbar } from "../components/WorkspaceToolbar";
import { WorkspaceTabs } from "../components/WorkspaceTabs";
import { WorkspaceTable } from "../components/WorkspaceTable";
import { WorkspaceDetail } from "../components/WorkspaceDetail";
import { AssociationPanel } from "../components/AssociationPanel";
import { WorkspaceStatsBar } from "../components/WorkspaceStatsBar";
import { RuleManagerDialog } from "../components/RuleManagerDialog";
import { Button } from "@/components/ui/button";
import { exportCsv } from "@/lib/csv";

export function WorkspacePage() {
  const { t } = useTranslation();
  useWorkspaceAutoruns();
  useWorkspaceNetwork();
  useWorkspaceEvents();

  const refreshMutation = useWorkspaceRefresh();
  const doSearch = useWorkspaceSearch();
  const doRuleScan = useWorkspaceRuleScan();

  const activeTab = useWorkspaceStore((s) => s.activeTab);
  const autorunItems = useWorkspaceStore((s) => s.autorunItems);
  const networkItems = useWorkspaceStore((s) => s.networkItems);
  const eventItems = useWorkspaceStore((s) => s.eventItems);
  const filteredAutorunIds = useWorkspaceStore((s) => s.filteredAutorunIds);
  const filteredNetworkKeys = useWorkspaceStore((s) => s.filteredNetworkKeys);
  const filteredEventKeys = useWorkspaceStore((s) => s.filteredEventKeys);
  const rules = useWorkspaceStore((s) => s.rules);
  const setRules = useWorkspaceStore((s) => s.setRules);
  const scanning = useWorkspaceStore((s) => s.scanning);
  const setScanning = useWorkspaceStore((s) => s.setScanning);
  const selectedAutorunId = useWorkspaceStore((s) => s.selectedAutorunId);
  const selectedNetworkKey = useWorkspaceStore((s) => s.selectedNetworkKey);
  const selectedEventKey = useWorkspaceStore((s) => s.selectedEventKey);
  const clearResults = useWorkspaceStore((s) => s.clearResults);

  const [ruleManagerOpen, setRuleManagerOpen] = useState(false);
  const [showAssociation, setShowAssociation] = useState(false);

  // Load rules on mount
  useEffect(() => {
    setRules(loadRules());
  }, [setRules]);

  // Derived counts
  const autorunFiltered = useMemo(
    () => (filteredAutorunIds ? autorunItems.filter((i) => filteredAutorunIds.has(i.id)).length : autorunItems.length),
    [autorunItems, filteredAutorunIds]
  );
  const networkFiltered = useMemo(
    () => (filteredNetworkKeys ? networkItems.filter((i) => filteredNetworkKeys.has(networkKey(i))).length : networkItems.length),
    [networkItems, filteredNetworkKeys]
  );
  const eventFiltered = useMemo(
    () => (filteredEventKeys ? eventItems.filter((i) => filteredEventKeys.has(eventKey(i))).length : eventItems.length),
    [eventItems, filteredEventKeys]
  );

  const hasAnyData = autorunItems.length > 0 || networkItems.length > 0 || eventItems.length > 0;

  const handleRefresh = useCallback(() => {
    refreshMutation.mutate();
  }, [refreshMutation]);

  const handleSearch = useCallback((query: string) => {
    doSearch(query);
  }, [doSearch]);

  const handleRuleScan = useCallback(() => {
    setScanning(true);
    try {
      doRuleScan(rules);
    } finally {
      setScanning(false);
    }
  }, [rules, doRuleScan, setScanning]);

  const handleReset = useCallback(() => {
    clearResults();
  }, [clearResults]);

  const handleExport = useCallback(async () => {
    if (activeTab === "autoruns" && autorunItems.length > 0) {
      const rows = autorunItems.map((a) => ({
        entry: a.entry,
        category: a.category,
        image_path: a.image_path || "",
        publisher: a.signature.kind === "valid" ? a.signature.detail.signer : a.signature.kind,
        enabled: a.enabled ? "是" : "否",
      }));
      await exportCsv(rows, ["entry", "category", "image_path", "publisher", "enabled"], `workspace-autoruns-${Date.now()}.csv`);
    } else if (activeTab === "network" && networkItems.length > 0) {
      const rows = networkItems.map((n) => ({
        protocol: n.proto,
        local: `${n.local.addr}:${n.local.port}`,
        remote: `${n.remote.addr}:${n.remote.port}`,
        state: n.state,
        pid: n.pid,
        process: n.process_name || "",
        path: n.process_path || "",
      }));
      await exportCsv(rows, ["protocol", "local", "remote", "state", "pid", "process", "path"], `workspace-network-${Date.now()}.csv`);
    } else if (activeTab === "events" && eventItems.length > 0) {
      const rows = eventItems.map((e) => ({
        timestamp: e.timestamp,
        event_type: e.event_type,
        process: e.process_name,
        source: `${e.source_ip}:${e.source_port}`,
        destination: `${e.destination_ip}:${e.destination_port}`,
        protocol: e.protocol,
      }));
      await exportCsv(rows, ["timestamp", "event_type", "process", "source", "destination", "protocol"], `workspace-events-${Date.now()}.csv`);
    }
  }, [activeTab, autorunItems, networkItems, eventItems]);

  // Get the currently selected item for association
  const selectedSourceItem = useMemo(() => {
    switch (activeTab) {
      case "autoruns":
        return selectedAutorunId != null ? autorunItems.find((a) => a.id === selectedAutorunId) ?? null : null;
      case "network":
        return selectedNetworkKey != null ? networkItems.find((n) => networkKey(n) === selectedNetworkKey) ?? null : null;
      case "events":
        return selectedEventKey != null ? eventItems.find((e) => eventKey(e) === selectedEventKey) ?? null : null;
    }
  }, [activeTab, selectedAutorunId, selectedNetworkKey, selectedEventKey, autorunItems, networkItems, eventItems]);

  const hasSelection = selectedAutorunId != null || selectedNetworkKey != null || selectedEventKey != null;

  const handleAssociation = useCallback(() => {
    setShowAssociation((prev) => !prev);
  }, []);

  if (!hasAnyData) {
    return (
      <div className="h-full flex flex-col items-center justify-center text-fg-tertiary gap-3">
        <p className="text-sm">{t("workspace.empty.message")}</p>
        <Button variant="secondary" size="sm" onClick={handleRefresh} disabled={refreshMutation.isPending}>
          <RefreshCw className={`h-3.5 w-3.5 mr-1 ${refreshMutation.isPending ? "animate-spin" : ""}`} />
          {t("workspace.empty.refresh")}
        </Button>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <WorkspaceToolbar
        onSearch={handleSearch}
        onRuleScan={handleRuleScan}
        onRuleManager={() => setRuleManagerOpen(true)}
        onRefresh={handleRefresh}
        onReset={handleReset}
        onExport={handleExport}
        scanning={scanning || refreshMutation.isPending}
      />

      <WorkspaceTabs
        autorunCount={autorunFiltered}
        networkCount={networkFiltered}
        eventCount={eventFiltered}
      />

      <div className="flex-1 min-h-0">
        <Group orientation="horizontal">
          <Panel defaultSize={60} minSize={30}>
            <WorkspaceTable onRowSelect={() => {}} />
          </Panel>
          {hasSelection && (
            <>
              <Separator className="w-px bg-border hover:bg-accent transition-colors" />
              <Panel defaultSize={40} minSize={20}>
                <div className="h-full flex flex-col">
                  <div className="flex-1 min-h-0 overflow-auto">
                    <WorkspaceDetail onAssociation={handleAssociation} />
                  </div>
                  {showAssociation && selectedSourceItem && (
                    <AssociationPanel sourceTab={activeTab} sourceItem={selectedSourceItem} />
                  )}
                </div>
              </Panel>
            </>
          )}
        </Group>
      </div>

      <WorkspaceStatsBar
        autorunTotal={autorunItems.length}
        autorunFiltered={autorunFiltered}
        networkTotal={networkItems.length}
        networkFiltered={networkFiltered}
        eventTotal={eventItems.length}
        eventFiltered={eventFiltered}
      />

      <RuleManagerDialog open={ruleManagerOpen} onOpenChange={setRuleManagerOpen} />
    </div>
  );
}
