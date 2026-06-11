import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

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
  readIds: Set<string>;
  highlightedAlertKey: string | null;
  alertPanelAutoOpen: boolean;
  addAlert: (alert: Alert) => void;
  markAllRead: () => void;
  markRead: (key: string) => void;
  clearAlerts: () => void;
  loadRecentAlerts: () => Promise<void>;
  setHighlightedAlertKey: (key: string | null) => void;
  setAlertPanelAutoOpen: (open: boolean) => void;
}

export function alertKey(a: { id: number; timestamp: number }): string {
  return `${a.id}-${a.timestamp}`;
}

// Route mapping: event_type → page
const NETWORK_EVENT_TYPES = new Set([
  "dns", "dns_client", "dns_pcap", "tls_sni", "network_connect", "network_monitor",
]);

function getPageForEventType(eventType: string): string {
  if (NETWORK_EVENT_TYPES.has(eventType)) return "/network";
  return "/log-collector";
}

export const useAlertStore = create<AlertState>((set) => ({
  alerts: [],
  unreadCount: 0,
  readIds: new Set<string>(),
  highlightedAlertKey: null,
  alertPanelAutoOpen: false,
  addAlert: (alert) => {
    set((state) => {
      const key = alertKey(alert);
      const alreadyRead = state.readIds.has(key);
      return {
        alerts: [alert, ...state.alerts].slice(0, 100),
        unreadCount: alreadyRead ? state.unreadCount : state.unreadCount + 1,
      };
    });
  },
  markAllRead: () => set((state) => ({
    unreadCount: 0,
    readIds: new Set(state.alerts.map((a) => alertKey(a))),
  })),
  markRead: (key) => set((state) => {
    const newReadIds = new Set(state.readIds);
    const wasUnread = !newReadIds.has(key);
    newReadIds.add(key);
    return {
      readIds: newReadIds,
      unreadCount: wasUnread ? Math.max(0, state.unreadCount - 1) : state.unreadCount,
    };
  }),
  clearAlerts: () => {
    invoke("cmd_monitor_clear_alerts").catch(() => {});
    set({ alerts: [], unreadCount: 0, readIds: new Set<string>() });
  },
  loadRecentAlerts: async () => {
    try {
      const alerts = await invoke<Alert[]>("cmd_monitor_get_alerts", { limit: 50 });
      set({ alerts, readIds: new Set(alerts.map((a) => alertKey(a))) });
    } catch {}
  },
  setHighlightedAlertKey: (key) => set({ highlightedAlertKey: key }),
  setAlertPanelAutoOpen: (open) => set({ alertPanelAutoOpen: open }),
}));

// Setup listener — call once at app startup
let listenerSetup = false;
export async function setupAlertListener() {
  if (listenerSetup) return;
  listenerSetup = true;

  const store = useAlertStore.getState();

  // Load existing alerts
  store.loadRecentAlerts();

  // Listen for popup click from external alert popup window
  listen<{ alert_key: string; event_type?: string }>("evt_alert_popup_clicked", (event) => {
    const { alert_key, event_type } = event.payload;

    // Navigate to the relevant page
    if (event_type) {
      import("../router").then(({ router }) => {
        const page = getPageForEventType(event_type);
        router.navigate({ to: page }).catch(() => {});
      }).catch(() => {});
    }

    // Show and focus main window (bring to front even when minimized or behind other windows)
    const win = getCurrentWindow();
    win.setAlwaysOnTop(true)
      .then(() => win.show())
      .then(() => win.setFocus())
      .then(() => new Promise<void>((resolve) => setTimeout(resolve, 50)))
      .then(() => win.setAlwaysOnTop(false))
      .catch(() => {});

    // Open alert panel and highlight
    const state = useAlertStore.getState();
    state.setHighlightedAlertKey(alert_key);
    state.setAlertPanelAutoOpen(true);
  });

  // Listen for new alerts
  listen<Alert>("evt_monitor_alert", (event) => {
    const alert = event.payload;
    useAlertStore.getState().addAlert(alert);

    // Show external alert popup window (visible even when app is minimized)
    try {
      const processName = (() => {
        try {
          const r = JSON.parse(alert.raw_json);
          const pid = r.process_id || r.pid;
          return pid ? `${alert.process_name} (${pid})` : alert.process_name;
        } catch { return alert.process_name; }
      })();
      const protocol = (() => {
        try {
          const r = JSON.parse(alert.raw_json);
          if (r.protocol) return r.protocol.toUpperCase();
          if (r.proto) return r.proto.toUpperCase();
          if (r.event_kind === "tls_sni") return "TCP";
          if (r.event_kind === "dns_query") return "UDP";
          return "";
        } catch { return ""; }
      })();
      // Extract source_addr from raw_json
      const sourceAddr = (() => {
        try {
          const r = JSON.parse(alert.raw_json);
          if (r.source_ip) return `${r.source_ip}${r.source_port ? `:${r.source_port}` : ""}`;
          if (r.src_ip) return `${r.src_ip}${r.src_port ? `:${r.src_port}` : ""}`;
          if (r.local?.addr) return `${r.local.addr}${r.local.port ? `:${r.local.port}` : ""}`;
          return "";
        } catch { return ""; }
      })();
      // Extract remote_addr from raw_json
      const remoteAddr = (() => {
        try {
          const r = JSON.parse(alert.raw_json);
          if (r.destination_ip) return `${r.destination_ip}${r.destination_port ? `:${r.destination_port}` : ""}`;
          if (r.dst_ip) return `${r.dst_ip}${r.dst_port ? `:${r.dst_port}` : ""}`;
          if (r.remote?.addr) return `${r.remote.addr}${r.remote.port ? `:${r.remote.port}` : ""}`;
          return "";
        } catch { return ""; }
      })();
      // Extract process_chain from raw_json
      const processChain = (() => {
        try {
          const r = JSON.parse(alert.raw_json);
          return r.process_chain ?? "";
        } catch { return ""; }
      })();
      invoke("cmd_show_alert_popup", {
        params: {
          rule_name: alert.rule_name,
          key_field: alert.key_field,
          event_type: alert.event_type,
          process_name: processName,
          protocol,
          timestamp: alert.timestamp,
          source_addr: sourceAddr || null,
          remote_addr: remoteAddr || null,
          process_chain: processChain || null,
        },
      }).catch(() => {});
    } catch {
    }
  });
}
