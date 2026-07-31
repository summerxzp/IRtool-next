import type { SysmonEventType } from "@/lib/bindings";
export type { SysmonEvent, SysmonEventType, SysmonStatus, EventConfigEntry } from "@/lib/bindings";

/** Extended event type including pcap-derived events not in the auto-generated SysmonEventType. */
export type ExtendedSysmonEventType = SysmonEventType | "tls_sni" | "dns_pcap" | "network_monitor";

export interface LogCollectorFilters {
  eventTypes: ExtendedSysmonEventType[];
  externalOnly: boolean;
  search: string;
}

export const EVENT_TYPE_LABELS: Record<ExtendedSysmonEventType, string> = {
  process_create: "进程创建",
  file_create_time: "文件创建时间修改",
  network_connect: "网络连接",
  process_terminate: "进程终止",
  driver_load: "驱动加载",
  image_load: "DLL加载",
  create_remote_thread: "远程线程",
  raw_access_read: "原始磁盘访问",
  process_access: "进程访问",
  file_create: "文件创建",
  registry_event: "注册表事件",
  file_create_stream_hash: "文件流哈希",
  pipe_event: "管道事件",
  wmi_event: "WMI事件",
  dns: "DNS查询",
  dns_client: "DNS-Client",
  file_delete: "文件删除",
  clipboard_change: "剪贴板变化",
  process_tampering: "进程篡改",
  file_delete_detected: "文件删除检测",
  unknown: "未知",
  tls_sni: "TLS SNI",
  dns_pcap: "DNS抓包",
  network_monitor: "网络监控",
};

export type EventSeverity = "default" | "info" | "warning" | "danger" | "critical";

export const EVENT_TYPE_SEVERITY: Record<ExtendedSysmonEventType, EventSeverity> = {
  process_create: "info",
  file_create_time: "warning",
  network_connect: "info",
  process_terminate: "default",
  driver_load: "danger",
  image_load: "warning",
  create_remote_thread: "danger",
  raw_access_read: "danger",
  process_access: "warning",
  file_create: "warning",
  registry_event: "warning",
  file_create_stream_hash: "warning",
  pipe_event: "info",
  wmi_event: "danger",
  dns: "info",
  dns_client: "info",
  file_delete: "danger",
  clipboard_change: "info",
  process_tampering: "critical",
  file_delete_detected: "danger",
  unknown: "default",
  tls_sni: "info",
  dns_pcap: "info",
  network_monitor: "info",
};

/** Tailwind class string for a badge representing the given severity. */
export function severityToBadgeClass(sev: EventSeverity): string {
  switch (sev) {
    case "critical":
    case "danger":
      return "bg-danger-bg text-danger border-danger-border";
    case "warning":
      return "bg-warning-bg text-warning border-warning-border";
    case "info":
      return "bg-info-bg text-accent border-info-border";
    case "default":
    default:
      return "bg-bg-elev-2 text-fg-secondary border-border";
  }
}

export const DEFAULT_ENABLED_EVENT_IDS = [3008, 22, 3]; // DNS Client + DNS + Network

/** Event record from the monitor database (backend MonitorEvent). */
export interface MonitorEvent {
  id: number;
  timestamp: number;
  source: "Sysmon" | "DnsClient" | "NetMonitor" | "Pcap";
  event_type: string;
  process_name: string;
  key_field: string;
  raw_json: string;
}

/** Paginated result from cmd_monitor_search_event_page. */
export interface EventPage {
  items: MonitorEvent[];
  total: number;
  limit: number;
  offset: number;
}
