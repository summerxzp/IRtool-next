import { create } from "zustand";
import type { SysmonEvent, SysmonStatus, LogCollectorFilters } from "./types";

const MAX_EVENTS = 10000;

interface LogCollectorState {
  events: SysmonEvent[];
  collecting: boolean;
  startTime: number | null;
  sysmonStatus: SysmonStatus | null;
  filters: LogCollectorFilters;
  selectedRecordId: number | null;
  autoScroll: boolean;

  addEvents: (events: SysmonEvent[]) => void;
  clearEvents: () => void;
  setCollecting: (v: boolean) => void;
  setStartTime: (t: number | null) => void;
  setSysmonStatus: (s: SysmonStatus) => void;
  setFilter: <K extends keyof LogCollectorFilters>(key: K, value: LogCollectorFilters[K]) => void;
  resetFilters: () => void;
  setSelectedRecordId: (id: number | null) => void;
  setAutoScroll: (v: boolean) => void;
}

const DEFAULT_FILTERS: LogCollectorFilters = {
  eventType: "all",
  externalOnly: false,
  search: "",
};

export const useLogCollectorStore = create<LogCollectorState>((set) => ({
  events: [],
  collecting: false,
  startTime: null,
  sysmonStatus: null,
  filters: DEFAULT_FILTERS,
  selectedRecordId: null,
  autoScroll: true,

  addEvents: (newEvents) =>
    set((s) => {
      const combined = [...s.events, ...newEvents];
      return { events: combined.length > MAX_EVENTS ? combined.slice(-MAX_EVENTS) : combined };
    }),

  clearEvents: () => set({ events: [] }),
  setCollecting: (collecting) => set({ collecting }),
  setStartTime: (startTime) => set({ startTime }),
  setSysmonStatus: (sysmonStatus) => set({ sysmonStatus }),
  setFilter: (key, value) => set((s) => ({ filters: { ...s.filters, [key]: value } })),
  resetFilters: () => set({ filters: DEFAULT_FILTERS }),
  setSelectedRecordId: (selectedRecordId) => set({ selectedRecordId }),
  setAutoScroll: (autoScroll) => set({ autoScroll }),
}));
