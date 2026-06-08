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
