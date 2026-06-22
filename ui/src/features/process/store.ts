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
}

export const useProcessStore = create<ProcessState>()((set) => ({
  selectedPid: null,
  setSelectedPid: (selectedPid) => set({ selectedPid }),

  viewMode: "list",
  setViewMode: (viewMode) => set({ viewMode }),

  filter: "all",
  setFilter: (filter) => set({ filter }),

  search: "",
  setSearch: (search) => set({ search }),
}));
