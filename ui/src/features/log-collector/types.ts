import type { SysmonEventType } from "@/lib/bindings";
export type { SysmonEvent, SysmonEventType, SysmonStatus, EventConfigEntry } from "@/lib/bindings";

/** Extended event type including pcap-derived events not in the auto-generated SysmonEventType. */
export type ExtendedSysmonEventType = SysmonEventType | "tls_sni" | "dns_pcap";

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
};

export const EVENT_TYPE_COLORS: Record<ExtendedSysmonEventType, string> = {
  process_create: "bg-purple-500/15 text-purple-500 border-purple-500/25",
  file_create_time: "bg-amber-500/15 text-amber-500 border-amber-500/25",
  network_connect: "bg-green-500/15 text-green-500 border-green-500/25",
  process_terminate: "bg-gray-500/15 text-gray-500 border-gray-500/25",
  driver_load: "bg-red-500/15 text-red-500 border-red-500/25",
  image_load: "bg-pink-500/15 text-pink-500 border-pink-500/25",
  create_remote_thread: "bg-red-500/15 text-red-500 border-red-500/25",
  raw_access_read: "bg-red-500/15 text-red-500 border-red-500/25",
  process_access: "bg-orange-500/15 text-orange-500 border-orange-500/25",
  file_create: "bg-orange-500/15 text-orange-500 border-orange-500/25",
  registry_event: "bg-yellow-500/15 text-yellow-500 border-yellow-500/25",
  file_create_stream_hash: "bg-amber-500/15 text-amber-500 border-amber-500/25",
  pipe_event: "bg-cyan-500/15 text-cyan-500 border-cyan-500/25",
  wmi_event: "bg-violet-500/15 text-violet-500 border-violet-500/25",
  dns: "bg-blue-500/15 text-blue-500 border-blue-500/25",
  dns_client: "bg-indigo-500/15 text-indigo-500 border-indigo-500/25",
  file_delete: "bg-rose-500/15 text-rose-500 border-rose-500/25",
  clipboard_change: "bg-teal-500/15 text-teal-500 border-teal-500/25",
  process_tampering: "bg-red-500/15 text-red-500 border-red-500/25",
  file_delete_detected: "bg-rose-500/15 text-rose-500/25",
  unknown: "bg-gray-500/15 text-gray-500 border-gray-500/25",
  tls_sni: "bg-sky-500/15 text-sky-500 border-sky-500/25",
  dns_pcap: "bg-lime-500/15 text-lime-500 border-lime-500/25",
};

export const DEFAULT_ENABLED_EVENT_IDS = [3008, 22, 3]; // DNS Client + DNS + Network
