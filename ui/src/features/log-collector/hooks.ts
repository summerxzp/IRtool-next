import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQuery, useMutation } from "@tanstack/react-query";
import { toast } from "sonner";
import * as api from "./api";
import { useLogCollectorStore } from "./store";
import { DEFAULT_ENABLED_EVENT_IDS } from "./types";
import type { SysmonEvent, ExtendedSysmonEventType, EventPage } from "./types";

const EVT_SYSMON_EVENT = "evt_sysmon_event";
const EVT_PCAP_EVENT = "evt_pcap_event";

/** Shape of the PcapEvent emitted by the Rust backend. */
interface PcapEvent {
  timestamp: number;
  event_kind: "tls_sni" | "dns_query";
  domain: string;
  src_ip: string;
  src_port: number;
  dst_ip: string;
  dst_port: number;
  query_type: string;
}

function formatTimestamp(epochMs: number): string {
  const d = new Date(epochMs);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function pcapEventToSysmonEvent(pe: PcapEvent): SysmonEvent {
  const eventType: ExtendedSysmonEventType = pe.event_kind === "tls_sni" ? "tls_sni" : "dns_pcap";
  return {
    event_id: 0,
    event_type: eventType as SysmonEvent["event_type"],
    timestamp: formatTimestamp(pe.timestamp),
    timestamp_epoch: pe.timestamp / 1000,
    timestamp_valid: true,
    record_id: null,
    raw_data: {},
    process_id: 0,
    process_name: "",
    process_path: "",
    user: "",
    rule_name: "",
    query_name: pe.domain,
    query_results: pe.event_kind === "dns_query" ? pe.query_type : "",
    query_status: 0,
    source_ip: pe.src_ip,
    source_port: pe.src_port,
    destination_ip: pe.dst_ip,
    destination_port: pe.dst_port,
    protocol: pe.event_kind === "tls_sni" ? "tcp" : "udp",
    initiated: true,
    is_external: false,
    source_process_id: 0,
    source_process_name: "",
    source_process_path: "",
    target_process_id: 0,
    target_process_name: "",
    target_process_path: "",
    start_address: "",
    start_module: "",
    start_function: "",
    is_suspicious: false,
    target_filename: "",
    creation_utc_time: "",
  };
}

export function useSysmonStatus() {
  return useQuery({
    queryKey: ["sysmon", "status"],
    queryFn: api.getStatus,
    refetchInterval: 5000,
  });
}

export function useDefaultEventConfigs() {
  return useQuery({
    queryKey: ["sysmon", "event-configs"],
    queryFn: api.getDefaultEventConfigs,
  });
}

export function useLoadHistory() {
  return useMutation({
    mutationFn: ({ limit, enabledEventIds }: { limit: number; enabledEventIds: number[] }) =>
      api.getExistingEvents(limit, enabledEventIds),
  });
}

export function useInstallSysmon() {
  return useMutation({
    mutationFn: (acceptEula: boolean) => api.install(acceptEula),
  });
}

export function useUninstallSysmon() {
  return useMutation({
    mutationFn: () => api.uninstall(),
  });
}

export function useUpdateSysmonConfig() {
  return useMutation({
    mutationFn: () => api.updateConfig(),
  });
}

export function useSysmonEventListener() {
  const addEvents = useLogCollectorStore((s) => s.addEvents);
  const unlistenRef = useRef<(() => void) | null>(null);
  const batchRef = useRef<SysmonEvent[]>([]);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const MAX_BATCH_SIZE = 100;

    const flush = () => {
      if (batchRef.current.length > 0) {
        addEvents(batchRef.current);
        batchRef.current = [];
      }
      timerRef.current = null;
    };

    const setup = async () => {
      unlistenRef.current = await listen<SysmonEvent>(EVT_SYSMON_EVENT, (event) => {
        batchRef.current.push(event.payload);
        if (batchRef.current.length >= MAX_BATCH_SIZE) {
          if (timerRef.current) {
            clearTimeout(timerRef.current);
            timerRef.current = null;
          }
          flush();
        } else if (!timerRef.current) {
          timerRef.current = setTimeout(flush, 100);
        }
      });
    };
    setup();
    return () => {
      unlistenRef.current?.();
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        flush();
      }
    };
  }, [addEvents]);
}

export function usePcapEventListener() {
  const addEvents = useLogCollectorStore((s) => s.addEvents);
  const unlistenRef = useRef<(() => void) | null>(null);
  const batchRef = useRef<SysmonEvent[]>([]);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const MAX_BATCH_SIZE = 100;

    const flush = () => {
      if (batchRef.current.length > 0) {
        addEvents(batchRef.current);
        batchRef.current = [];
      }
      timerRef.current = null;
    };

    const setup = async () => {
      unlistenRef.current = await listen<PcapEvent>(EVT_PCAP_EVENT, (event) => {
        batchRef.current.push(pcapEventToSysmonEvent(event.payload));
        if (batchRef.current.length >= MAX_BATCH_SIZE) {
          if (timerRef.current) {
            clearTimeout(timerRef.current);
            timerRef.current = null;
          }
          flush();
        } else if (!timerRef.current) {
          timerRef.current = setTimeout(flush, 100);
        }
      });
    };
    setup();
    return () => {
      unlistenRef.current?.();
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        flush();
      }
    };
  }, [addEvents]);
}

export function useStartCollection(enabledEventIds: number[] = DEFAULT_ENABLED_EVENT_IDS) {
  const setCollecting = useLogCollectorStore((s) => s.setCollecting);
  const setStartTime = useLogCollectorStore((s) => s.setStartTime);
  const setAutoScroll = useLogCollectorStore((s) => s.setAutoScroll);

  return useMutation({
    mutationFn: async () => {
      await api.startSubscription(enabledEventIds, 500);
      // 启动 pcap（根据保存的配置）
      try {
        const config = await api.monitorGetConfig();
        if (config.enable_sni || config.enable_dns_pcap) {
          // 后台模式下 PCAP 开销较大，给出警告
          const isBackground = await api.monitorIsBackground();
          if (isBackground) {
            toast.warning("PCAP 在后台模式下开销较大，建议仅在深度捕获模式下启用");
          }
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("cmd_pcap_start", {
            config: {
              enable_sni: config.enable_sni,
              enable_dns_pcap: config.enable_dns_pcap,
              adapter_ip: config.adapter_ip ?? null,
              max_duration_secs: config.max_duration_secs ?? 0,
            },
          });
        }
      } catch {
        // pcap 启动失败不影响主流程
      }
    },
    onSuccess: () => {
      setCollecting(true);
      setStartTime(Date.now());
      setAutoScroll(true);
    },
  });
}

export function useStopCollection() {
  const setCollecting = useLogCollectorStore((s) => s.setCollecting);
  const setStartTime = useLogCollectorStore((s) => s.setStartTime);

  return useMutation({
    mutationFn: async () => {
      await api.stopSubscription();
      // 停止 pcap
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("cmd_pcap_stop");
      } catch {
        // pcap 停止失败不影响主流程
      }
    },
    onSuccess: () => {
      setCollecting(false);
      setStartTime(null);
    },
  });
}

export function useLogMaxSize() {
  return useQuery({
    queryKey: ["sysmon", "log-max-size"],
    queryFn: api.getLogMaxSize,
    refetchInterval: 30000,
  });
}

export function useSetLogMaxSize() {
  return useMutation({
    mutationFn: (sizeMb: number) => api.setLogMaxSize(sizeMb),
  });
}

/** Sync frontend collecting state with backend on mount. */
export function useSyncCollectingState() {
  const setCollecting = useLogCollectorStore((s) => s.setCollecting);

  useEffect(() => {
    (async () => {
      try {
        const subscribing = await api.isSubscribing();
        setCollecting(subscribing);
      } catch {
        // ignore
      }
    })();
  }, [setCollecting]);
}

/** Paginated history query via backend cmd_monitor_search_event_page. */
export function useSearchEventPage() {
  return useMutation({
    mutationFn: async (params: {
      source?: string | null;
      event_type?: string | null;
      process_name?: string | null;
      key_field?: string | null;
      is_external?: boolean | null;
      search_text?: string | null;
      limit?: number;
      offset?: number;
    }): Promise<EventPage> => {
      return api.searchEventPage(params);
    },
  });
}


