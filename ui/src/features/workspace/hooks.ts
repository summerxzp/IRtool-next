import { useEffect } from "react";
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
 * Uses useEffect to sync query data into store, avoiding render-time side effects.
 */
export function useWorkspaceAutoruns() {
  const setAutorunItems = useWorkspaceStore((s) => s.setAutorunItems);
  const query = useQuery({
    queryKey: QK_WORKSPACE_AUTORUNS,
    queryFn: async () => {
      const items = await api.getAutorunItems();
      return items;
    },
    staleTime: 30_000,
  });

  useEffect(() => {
    if (query.data) {
      setAutorunItems(query.data);
    }
  }, [query.data, setAutorunItems]);

  return query;
}

/**
 * Fetch network snapshot for workspace.
 * Uses useEffect to sync query data into store, avoiding render-time side effects.
 */
export function useWorkspaceNetwork() {
  const setNetworkItems = useWorkspaceStore((s) => s.setNetworkItems);
  const query = useQuery({
    queryKey: QK_WORKSPACE_NETWORK,
    queryFn: async () => {
      const payload = await api.getNetworkSnapshot();
      return payload.items;
    },
    staleTime: 30_000,
  });

  useEffect(() => {
    if (query.data) {
      setNetworkItems(query.data);
    }
  }, [query.data, setNetworkItems]);

  return query;
}

/**
 * Sync log collector events into workspace store.
 * Uses useEffect to avoid render-time side effects that cause infinite loops.
 */
export function useWorkspaceEvents() {
  const setEventItems = useWorkspaceStore((s) => s.setEventItems);
  const events = useLogCollectorStore((s) => s.events);

  useEffect(() => {
    setEventItems(events);
  }, [events, setEventItems]);

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
 * Uses getState() to avoid subscribing to the entire store.
 */
export function useWorkspaceSearch() {
  const setFilteredAutorunIds = useWorkspaceStore((s) => s.setFilteredAutorunIds);
  const setFilteredNetworkKeys = useWorkspaceStore((s) => s.setFilteredNetworkKeys);
  const setFilteredEventKeys = useWorkspaceStore((s) => s.setFilteredEventKeys);
  const setSearchQuery = useWorkspaceStore((s) => s.setSearchQuery);

  return (query: string) => {
    const { autorunItems, networkItems, eventItems } = useWorkspaceStore.getState();
    const filteredAutorunIds = ruleEngine.searchAutoruns(autorunItems, query);
    const filteredNetworkKeys = ruleEngine.searchNetwork(networkItems, query);
    const filteredEventKeys = ruleEngine.searchEvents(eventItems, query);
    setFilteredAutorunIds(filteredAutorunIds);
    setFilteredNetworkKeys(filteredNetworkKeys);
    setFilteredEventKeys(filteredEventKeys);
    setSearchQuery(query);
  };
}

/**
 * Execute rule scan across all data sources.
 * Uses getState() to avoid subscribing to the entire store.
 */
export function useWorkspaceRuleScan() {
  const setFilteredAutorunIds = useWorkspaceStore((s) => s.setFilteredAutorunIds);
  const setFilteredNetworkKeys = useWorkspaceStore((s) => s.setFilteredNetworkKeys);
  const setFilteredEventKeys = useWorkspaceStore((s) => s.setFilteredEventKeys);
  const setAutorunMatchedRules = useWorkspaceStore((s) => s.setAutorunMatchedRules);
  const setNetworkMatchedRules = useWorkspaceStore((s) => s.setNetworkMatchedRules);
  const setEventMatchedRules = useWorkspaceStore((s) => s.setEventMatchedRules);
  const setSearchQuery = useWorkspaceStore((s) => s.setSearchQuery);

  return (rules: Rule[]) => {
    const { autorunItems, networkItems, eventItems } = useWorkspaceStore.getState();
    const autorunMatchedRules = ruleEngine.scanAutoruns(autorunItems, rules);
    const networkMatchedRules = ruleEngine.scanNetwork(networkItems, rules);
    const eventMatchedRules = ruleEngine.scanEvents(eventItems, rules);

    const filteredAutorunIds = new Set(autorunMatchedRules.keys());
    const filteredNetworkKeys = new Set(networkMatchedRules.keys());
    const filteredEventKeys = new Set(eventMatchedRules.keys());

    setFilteredAutorunIds(filteredAutorunIds);
    setFilteredNetworkKeys(filteredNetworkKeys);
    setFilteredEventKeys(filteredEventKeys);
    setAutorunMatchedRules(autorunMatchedRules);
    setNetworkMatchedRules(networkMatchedRules);
    setEventMatchedRules(eventMatchedRules);
    setSearchQuery(`[规则扫描] ${rules.filter((r) => r.enabled).length} 条规则`);
  };
}

/**
 * Unhide a file or directory (remove hidden attribute).
 */
export function useUnhidePath() {
  return useMutation({
    mutationFn: (path: string) => api.unhidePath(path),
  });
}

/**
 * Take ownership of a file or directory.
 */
export function useTakeOwnership() {
  return useMutation({
    mutationFn: (path: string) => api.takeOwnership(path),
  });
}

/**
 * Sample a file (zip with password protection).
 */
export function useSamplePath() {
  return useMutation({
    mutationFn: ({ path, outputDir, password }: { path: string; outputDir: string; password: string }) =>
      api.samplePath(path, outputDir, password),
  });
}

/**
 * Open a path in explorer.
 */
export function useOpenPath() {
  return useMutation({
    mutationFn: (path: string) => api.openPath(path),
  });
}
