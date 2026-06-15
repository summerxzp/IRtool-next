import { create } from "zustand";

interface MonitoringState {
  isBackground: boolean;
  telemetry: {
    mode: string;
    started_at: number | null;
    events_written: number;
    events_dropped: number;
    last_event_at: number | null;
    last_error: string | null;
  } | null;
  eventCount: number;
  dbSize: number; // 数据库大小（字节）

  setIsBackground: (v: boolean) => void;
  setTelemetry: (t: MonitoringState["telemetry"]) => void;
  setEventCount: (c: number) => void;
  setDbSize: (s: number) => void;
}

export const useMonitoringStore = create<MonitoringState>()((set) => ({
  isBackground: false,
  telemetry: null,
  eventCount: 0,
  dbSize: 0,
  setIsBackground: (isBackground) => set({ isBackground }),
  setTelemetry: (telemetry) => set({ telemetry }),
  setEventCount: (eventCount) => set({ eventCount }),
  setDbSize: (dbSize) => set({ dbSize }),
}));
