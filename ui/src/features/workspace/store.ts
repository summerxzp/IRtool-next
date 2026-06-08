import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { WorkspaceTab, Rule } from "./types";
import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";

interface WorkspaceState {
  // Tab
  activeTab: WorkspaceTab;
  setActiveTab: (tab: WorkspaceTab) => void;

  // Search
  searchQuery: string;
  setSearchQuery: (q: string) => void;

  // Data — raw from sources
  autorunItems: AutorunItem[];
  networkItems: NetConn[];
  eventItems: SysmonEvent[];
  setAutorunItems: (items: AutorunItem[]) => void;
  setNetworkItems: (items: NetConn[]) => void;
  setEventItems: (items: SysmonEvent[]) => void;

  // Data — filtered/search results (null = no filter active, show all)
  filteredAutorunIds: Set<number> | null;
  filteredNetworkKeys: Set<string> | null;
  filteredEventKeys: Set<string> | null;
  setFilteredAutorunIds: (ids: Set<number> | null) => void;
  setFilteredNetworkKeys: (keys: Set<string> | null) => void;
  setFilteredEventKeys: (keys: Set<string> | null) => void;

  // Data — rule scan results
  autorunMatchedRules: Map<number, Rule[]>;
  networkMatchedRules: Map<string, Rule[]>;
  eventMatchedRules: Map<string, Rule[]>;
  setAutorunMatchedRules: (m: Map<number, Rule[]>) => void;
  setNetworkMatchedRules: (m: Map<string, Rule[]>) => void;
  setEventMatchedRules: (m: Map<string, Rule[]>) => void;

  // Selection
  selectedAutorunId: number | null;
  selectedNetworkKey: string | null;
  selectedEventKey: string | null;
  setSelectedAutorunId: (id: number | null) => void;
  setSelectedNetworkKey: (key: string | null) => void;
  setSelectedEventKey: (key: string | null) => void;

  // Rules
  rules: Rule[];
  setRules: (rules: Rule[]) => void;

  // Scan state
  scanning: boolean;
  setScanning: (v: boolean) => void;

  // Loading
  loading: boolean;
  setLoading: (v: boolean) => void;

  // Clear all results
  clearResults: () => void;
}

export const useWorkspaceStore = create<WorkspaceState>()(
  persist(
    (set) => ({
      activeTab: "autoruns",
      setActiveTab: (activeTab) => set({ activeTab }),

      searchQuery: "",
      setSearchQuery: (searchQuery) => set({ searchQuery }),

      autorunItems: [],
      networkItems: [],
      eventItems: [],
      setAutorunItems: (autorunItems) => set({ autorunItems }),
      setNetworkItems: (networkItems) => set({ networkItems }),
      setEventItems: (eventItems) => set({ eventItems }),

      filteredAutorunIds: null,
      filteredNetworkKeys: null,
      filteredEventKeys: null,
      setFilteredAutorunIds: (filteredAutorunIds) => set({ filteredAutorunIds }),
      setFilteredNetworkKeys: (filteredNetworkKeys) => set({ filteredNetworkKeys }),
      setFilteredEventKeys: (filteredEventKeys) => set({ filteredEventKeys }),

      autorunMatchedRules: new Map(),
      networkMatchedRules: new Map(),
      eventMatchedRules: new Map(),
      setAutorunMatchedRules: (autorunMatchedRules) => set({ autorunMatchedRules }),
      setNetworkMatchedRules: (networkMatchedRules) => set({ networkMatchedRules }),
      setEventMatchedRules: (eventMatchedRules) => set({ eventMatchedRules }),

      selectedAutorunId: null,
      selectedNetworkKey: null,
      selectedEventKey: null,
      setSelectedAutorunId: (selectedAutorunId) => set({ selectedAutorunId }),
      setSelectedNetworkKey: (selectedNetworkKey) => set({ selectedNetworkKey }),
      setSelectedEventKey: (selectedEventKey) => set({ selectedEventKey }),

      rules: [],
      setRules: (rules) => set({ rules }),

      scanning: false,
      setScanning: (scanning) => set({ scanning }),

      loading: false,
      setLoading: (loading) => set({ loading }),

      clearResults: () =>
        set({
          filteredAutorunIds: null,
          filteredNetworkKeys: null,
          filteredEventKeys: null,
          autorunMatchedRules: new Map(),
          networkMatchedRules: new Map(),
          eventMatchedRules: new Map(),
          searchQuery: "",
        }),
    }),
    {
      name: "irtool-workspace",
      partialize: (state) =>
        ({
          rules: state.rules,
        }) as WorkspaceState,
    }
  )
);
