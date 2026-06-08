import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "./api";
import { useWorkspaceStore } from "./store";
import { useLogCollectorStore } from "@/features/log-collector/store";
import * as ruleEngine from "./rules/engine";
import type { Rule } from "./types";

const QK_WORKSPACE_AUTORUNS = ["workspace", "autoruns"] as const;
const QK_WORKSPACE_NETWORK = ["workspace", "network"] as const;

/**
 * Fetch autorun items for workspace.
 */
export function useWorkspaceAutoruns() {
  const setAutorunItems = useWorkspaceStore((s) => s.setAutorunItems);
  return useQuery({
    queryKey: QK_WORKSPACE_AUTORUNS,
    queryFn: async () => {
      const items = await api.getAutorunItems();
      setAutorunItems(items);
      return items;
    },
    staleTime: 0,
  });
}

/**
 * Fetch network snapshot for workspace.
 */
export function useWorkspaceNetwork() {
  const setNetworkItems = useWorkspaceStore((s) => s.setNetworkItems);
  return useQuery({
    queryKey: QK_WORKSPACE_NETWORK,
    queryFn: async () => {
      const payload = await api.getNetworkSnapshot();
      const items = payload.items;
      setNetworkItems(items);
      return items;
    },
    staleTime: 0,
  });
}

/**
 * Get log collector events from frontend store (no backend call needed).
 */
export function useWorkspaceEvents() {
  const setEventItems = useWorkspaceStore((s) => s.setEventItems);
  const events = useLogCollectorStore((s) => s.events);
  setEventItems(events);
  return events;
}

/**
 * Refresh all data sources in parallel.
 */
export function useWorkspaceRefresh() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const [autoruns, network] = await Promise.all([
        api.getAutorunItems(),
        api.getNetworkSnapshot(),
      ]);
      return { autoruns, network: network.items };
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: QK_WORKSPACE_AUTORUNS });
      qc.invalidateQueries({ queryKey: QK_WORKSPACE_NETWORK });
    },
  });
}

/**
 * Execute keyword search across all data sources.
 * Updates store with filtered sets.
 */
export function useWorkspaceSearch() {
  const store = useWorkspaceStore();
  return (query: string) => {
    const { autorunItems, networkItems, eventItems } = useWorkspaceStore.getState();
    const filteredAutorunIds = ruleEngine.searchAutoruns(autorunItems, query);
    const filteredNetworkKeys = ruleEngine.searchNetwork(networkItems, query);
    const filteredEventKeys = ruleEngine.searchEvents(eventItems, query);
    store.setFilteredAutorunIds(filteredAutorunIds);
    store.setFilteredNetworkKeys(filteredNetworkKeys);
    store.setFilteredEventKeys(filteredEventKeys);
    store.setSearchQuery(query);
  };
}

/**
 * Execute rule scan across all data sources.
 * Updates store with filtered sets and matched rules.
 */
export function useWorkspaceRuleScan() {
  const store = useWorkspaceStore();
  return (rules: Rule[]) => {
    const { autorunItems, networkItems, eventItems } = useWorkspaceStore.getState();
    const autorunMatchedRules = ruleEngine.scanAutoruns(autorunItems, rules);
    const networkMatchedRules = ruleEngine.scanNetwork(networkItems, rules);
    const eventMatchedRules = ruleEngine.scanEvents(eventItems, rules);

    const filteredAutorunIds = new Set(autorunMatchedRules.keys());
    const filteredNetworkKeys = new Set(networkMatchedRules.keys());
    const filteredEventKeys = new Set(eventMatchedRules.keys());

    store.setFilteredAutorunIds(filteredAutorunIds);
    store.setFilteredNetworkKeys(filteredNetworkKeys);
    store.setFilteredEventKeys(filteredEventKeys);
    store.setAutorunMatchedRules(autorunMatchedRules);
    store.setNetworkMatchedRules(networkMatchedRules);
    store.setEventMatchedRules(eventMatchedRules);
    store.setSearchQuery(`[规则扫描] ${rules.filter((r) => r.enabled).length} 条规则`);
  };
}

/**
 * Run a command template (attrib, takeown, 7z).
 */
export function useRunCommand() {
  return useMutation({
    mutationFn: ({ program, args }: { program: string; args: string }) =>
      api.runCommand(program, args),
  });
}
