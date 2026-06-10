import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { SysmonEvent, SysmonStatus, LogCollectorFilters } from "./types";

const MAX_EVENTS = 10000;
const PERSIST_EVENTS = 2000;

interface LogCollectorState {
  events: SysmonEvent[];
  seenKeys: Set<string>;
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
  eventTypes: [],
  externalOnly: false,
  search: "",
};

export const useLogCollectorStore = create<LogCollectorState>()(
  persist(
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
          if (combined.length > MAX_EVENTS) {
            // Remove oldest events (at the end)
            const removed = combined.slice(MAX_EVENTS);
            const newSeen = new Set(s.seenKeys);
            for (const e of removed) {
              newSeen.delete(`${e.record_id}-${e.timestamp}-${e.event_id}`);
            }
            return { events: combined.slice(0, MAX_EVENTS), seenKeys: newSeen };
          }
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
    }),
    {
      name: "irtool-log-collector",
      storage: {
        getItem: (name) => {
          const str = sessionStorage.getItem(name);
          if (!str) return null;
          const parsed = JSON.parse(str);
          if (parsed?.state?.seenKeys && Array.isArray(parsed.state.seenKeys)) {
            parsed.state.seenKeys = new Set(parsed.state.seenKeys);
          }
          return parsed;
        },
        setItem: (name, value) => {
          const state = value.state as LogCollectorState;
          const toStore = JSON.parse(JSON.stringify(value));
          // JSON.stringify converts Set to empty object, so replace with array
          if (toStore.state?.seenKeys && !(toStore.state.seenKeys instanceof Array)) {
            toStore.state.seenKeys = Array.from(state.seenKeys);
          }
          // Only persist last PERSIST_EVENTS events to avoid bloating sessionStorage
          // Since newest events are at top, keep first PERSIST_EVENTS
          if (toStore.state?.events?.length > PERSIST_EVENTS) {
            toStore.state.events = toStore.state.events.slice(0, PERSIST_EVENTS);
          }
          sessionStorage.setItem(name, JSON.stringify(toStore));
        },
        removeItem: (name) => sessionStorage.removeItem(name),
      },
      partialize: (state) =>
        ({
          events: state.events,
          seenKeys: state.seenKeys,
          enabledEventKeys: state.enabledEventKeys,
        }) as LogCollectorState,
    }
  )
);
