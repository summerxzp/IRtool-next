import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface Alert {
  id: number;
  timestamp: number;
  rule_name: string;
  event_type: string;
  process_name: string;
  key_field: string;
  action_taken: string;
  raw_json: string;
}

interface AlertState {
  alerts: Alert[];
  unreadCount: number;
  readIds: Set<number>;
  addAlert: (alert: Alert) => void;
  markAllRead: () => void;
  markRead: (id: number) => void;
  clearAlerts: () => void;
  loadRecentAlerts: () => Promise<void>;
}

export const useAlertStore = create<AlertState>((set) => ({
  alerts: [],
  unreadCount: 0,
  readIds: new Set<number>(),
  addAlert: (alert) => {
    set((state) => ({
      alerts: [alert, ...state.alerts].slice(0, 100),
      unreadCount: state.unreadCount + 1,
    }));
  },
  markAllRead: () => set((state) => ({
    unreadCount: 0,
    readIds: new Set(state.alerts.map((a) => a.id)),
  })),
  markRead: (id) => set((state) => {
    const newReadIds = new Set(state.readIds);
    newReadIds.add(id);
    return { readIds: newReadIds };
  }),
  clearAlerts: () => set({ alerts: [], unreadCount: 0, readIds: new Set() }),
  loadRecentAlerts: async () => {
    try {
      const alerts = await invoke<Alert[]>("cmd_monitor_get_alerts", { limit: 50 });
      set({ alerts, readIds: new Set(alerts.map((a) => a.id)) });
    } catch {}
  },
}));

// Setup listener — call once at app startup
let listenerSetup = false;
export function setupAlertListener() {
  if (listenerSetup) return;
  listenerSetup = true;

  // Request notification permission
  if ("Notification" in window && Notification.permission === "default") {
    Notification.requestPermission();
  }

  const store = useAlertStore.getState();

  // Load existing alerts
  store.loadRecentAlerts();

  // Listen for new alerts
  listen<Alert>("evt_monitor_alert", (event) => {
    const alert = event.payload;
    useAlertStore.getState().addAlert(alert);

    // Show OS notification
    try {
      if ('Notification' in window) {
        if (Notification.permission === 'granted') {
          new Notification(`🚨 ${alert.rule_name}`, {
            body: `目标: ${alert.key_field}\n进程: ${alert.process_name || "未知"} | 类型: ${alert.event_type}`,
            tag: `irtool-alert-${Date.now()}`,
          });
        } else if (Notification.permission !== 'denied') {
          Notification.requestPermission().then((perm) => {
            if (perm === 'granted') {
              new Notification(`🚨 ${alert.rule_name}`, {
                body: `目标: ${alert.key_field}\n进程: ${alert.process_name || "未知"} | 类型: ${alert.event_type}`,
              });
            }
          });
        }
      }
    } catch (e) {
      console.warn('Notification failed:', e);
    }
  });
}
