import type { SysmonEventType } from "@/lib/bindings";
export type { SysmonEvent, SysmonEventType, SysmonStatus, EventConfigEntry } from "@/lib/bindings";

export interface LogCollectorFilters {
  eventType: SysmonEventType | "all";
  externalOnly: boolean;
  search: string;
}

export const EVENT_TYPE_LABELS: Record<SysmonEventType, string> = {
  dns: "DNS查询",
  network_connect: "网络连接",
  create_remote_thread: "远程线程",
  file_create: "文件创建",
  unknown: "未知",
};

export const EVENT_TYPE_COLORS: Record<SysmonEventType, string> = {
  dns: "bg-blue-500/15 text-blue-500 border-blue-500/25",
  network_connect: "bg-green-500/15 text-green-500 border-green-500/25",
  create_remote_thread: "bg-red-500/15 text-red-500 border-red-500/25",
  file_create: "bg-orange-500/15 text-orange-500 border-orange-500/25",
  unknown: "bg-gray-500/15 text-gray-500 border-gray-500/25",
};

export const DEFAULT_ENABLED_EVENT_IDS = [22, 3]; // DNS + Network
