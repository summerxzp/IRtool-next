import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { ConnState, Proto, RetentionPolicyDto } from "./types";

export interface NetworkFilters {
  search: string;
  proto: Proto | "all";
  states: ConnState[];
  showHistory: boolean;
}

interface NetworkState {
  filters: NetworkFilters;
  setFilter: <K extends keyof NetworkFilters>(key: K, value: NetworkFilters[K]) => void;
  resetFilters: () => void;

  paused: boolean;
  setPaused: (paused: boolean) => void;

  intervalMs: number;
  setIntervalMs: (ms: number) => void;

  retention: RetentionPolicyDto;
  setRetention: (r: RetentionPolicyDto) => void;
}

const DEFAULT_FILTERS: NetworkFilters = {
  search: "",
  proto: "all",
  states: [],
  showHistory: true,
};

export const useNetworkStore = create<NetworkState>()(
  persist(
    (set) => ({
      filters: DEFAULT_FILTERS,
      setFilter: (key, value) =>
        set((s) => ({ filters: { ...s.filters, [key]: value } })),
      resetFilters: () => set({ filters: DEFAULT_FILTERS }),

      paused: false,
      setPaused: (paused) => set({ paused }),

      intervalMs: 1000,
      setIntervalMs: (ms) => set({ intervalMs: ms }),

      retention: { seconds: 600 } as RetentionPolicyDto,
      setRetention: (retention) => set({ retention }),
    }),
    {
      name: "irtool-network",
      partialize: () => ({}),
    }
  )
);
