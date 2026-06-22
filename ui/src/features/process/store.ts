import { create } from "zustand";
import type { ViewMode, FilterMode } from "./types";

interface ProcessState {
  selectedPid: number | null;
  setSelectedPid: (pid: number | null) => void;

  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;

  filter: FilterMode;
  setFilter: (f: FilterMode) => void;

  search: string;
  setSearch: (s: string) => void;

  expandAllVersion: number;
  toggleExpandAll: () => void;

  autoRefreshMs: number;
  setAutoRefreshMs: (ms: number) => void;
}

export const useProcessStore = create<ProcessState>()((set, get) => ({
  selectedPid: null,
  setSelectedPid: (selectedPid) => set({ selectedPid }),

  viewMode: "tree",
  setViewMode: (viewMode) => set({ viewMode }),

  filter: "all",
  setFilter: (filter) => set({ filter }),

  search: "",
  setSearch: (search) => set({ search }),

  expandAllVersion: 0,
  toggleExpandAll: () => set({ expandAllVersion: get().expandAllVersion + 1 }),

  autoRefreshMs: 0,
  setAutoRefreshMs: (autoRefreshMs) => set({ autoRefreshMs }),
}));
