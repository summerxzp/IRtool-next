import { create } from "zustand";
import type { SysmonEvent, SysmonStatus, LogCollectorFilters } from "./types";

interface LogCollectorState {
  // Live event buffer (foreground only, not persisted)
  events: SysmonEvent[];
  seenKeys: Set<string>;
  collecting: boolean;
  startTime: number | null;
  sysmonStatus: SysmonStatus | null;
  filters: LogCollectorFilters;
  selectedEvent: SysmonEvent | null;
  autoScroll: boolean;
  enabledEventKeys: string[];
  loadLimit: number;

  addEvents: (events: SysmonEvent[]) => void;
  clearEvents: () => void;
  setCollecting: (v: boolean) => void;
  setStartTime: (t: number | null) => void;
  setSysmonStatus: (s: SysmonStatus) => void;
  setFilter: <K extends keyof LogCollectorFilters>(key: K, value: LogCollectorFilters[K]) => void;
  resetFilters: () => void;
  setSelectedEvent: (event: SysmonEvent | null) => void;
  setAutoScroll: (v: boolean) => void;
  setEnabledEventKeys: (keys: string[]) => void;
  setLoadLimit: (limit: number) => void;
}

const DEFAULT_FILTERS: LogCollectorFilters = {
  eventTypes: [],
  externalOnly: false,
  search: "",
};

export const useLogCollectorStore = create<LogCollectorState>()(
  (set) => ({
    events: [],
    seenKeys: new Set<string>(),
    collecting: false,
    startTime: null,
    sysmonStatus: null,
    filters: DEFAULT_FILTERS,
    selectedEvent: null,
    autoScroll: true,
    enabledEventKeys: ["dns_client", "dns", "network_connect"],
    loadLimit: 5000,

    addEvents: (newEvents) =>
      set((s) => {
        if (newEvents.length === 0) return s;
        const uniqueNew = newEvents.filter((e) => {
          const key = `${e.record_id}-${e.timestamp}-${e.event_id}`;
          if (s.seenKeys.has(key)) return false;
          s.seenKeys.add(key);
          return true;
        });
        if (uniqueNew.length === 0) return s;
        // Prepend new events at top, keep newest
        const combined = [...uniqueNew, ...s.events];
        return { events: combined };
      }),

    clearEvents: () => set({ events: [], seenKeys: new Set(), selectedEvent: null }),
    setCollecting: (collecting) => set({ collecting }),
    setStartTime: (startTime) => set({ startTime }),
    setSysmonStatus: (sysmonStatus) => set({ sysmonStatus }),
    setFilter: (key, value) => set((s) => ({ filters: { ...s.filters, [key]: value } })),
    resetFilters: () => set({ filters: DEFAULT_FILTERS }),
    setSelectedEvent: (selectedEvent) => set({ selectedEvent }),
    setAutoScroll: (autoScroll) => set({ autoScroll }),
    setEnabledEventKeys: (enabledEventKeys) => set({ enabledEventKeys }),
    setLoadLimit: (loadLimit) => set({ loadLimit }),
  })
);
