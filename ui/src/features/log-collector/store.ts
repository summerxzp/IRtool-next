import { create } from "zustand";
import type { SysmonEvent, SysmonStatus, LogCollectorFilters } from "./types";

const MAX_EVENTS = 10000;

interface LogCollectorState {
  events: SysmonEvent[];
  collecting: boolean;
  startTime: number | null;
  sysmonStatus: SysmonStatus | null;
  filters: LogCollectorFilters;
  selectedEvent: SysmonEvent | null;
  autoScroll: boolean;
  enabledEventKeys: string[];

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
  selectedEvent: null,
  autoScroll: true,
  enabledEventKeys: ["dns_client", "dns", "network_connect"],

  addEvents: (newEvents) =>
    set((s) => {
      if (newEvents.length === 0) return s;
      const seen = new Set(s.events.map((e) => `${e.record_id}-${e.timestamp}-${e.event_id}`));
      const uniqueNew = newEvents.filter((e) => {
        const key = `${e.record_id}-${e.timestamp}-${e.event_id}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      if (uniqueNew.length === 0) return s;
      const combined = [...s.events, ...uniqueNew];
      return { events: combined.length > MAX_EVENTS ? combined.slice(-MAX_EVENTS) : combined };
    }),

  clearEvents: () => set({ events: [], selectedEvent: null }),
  setCollecting: (collecting) => set({ collecting }),
  setStartTime: (startTime) => set({ startTime }),
  setSysmonStatus: (sysmonStatus) => set({ sysmonStatus }),
  setFilter: (key, value) => set((s) => ({ filters: { ...s.filters, [key]: value } })),
  resetFilters: () => set({ filters: DEFAULT_FILTERS }),
  setSelectedEvent: (selectedEvent) => set({ selectedEvent }),
  setAutoScroll: (autoScroll) => set({ autoScroll }),
  setEnabledEventKeys: (enabledEventKeys) => set({ enabledEventKeys }),
}));
