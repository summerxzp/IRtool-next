import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";

// --- Rule Model ---

export type RuleTarget = "Autorun" | "Network" | "Event";

export type ConditionType = "contains" | "regex" | "equals";

export interface Condition {
  field: string;
  type: ConditionType;
  value: string;
}

export type Severity = "critical" | "high" | "medium" | "low";

export interface Rule {
  id: string;
  name: string;
  target: RuleTarget;
  conditions: Condition[];
  logic?: "and" | "or";
  severity: Severity;
  family: string;
  enabled: boolean;
  description?: string;
}

// --- Workspace Data Types ---

export type WorkspaceTab = "autoruns" | "network" | "events";

// --- Field Definitions per RuleTarget ---

export interface FieldDefinition {
  key: string;
  label: string;
}

export const AUTORUN_FIELDS: FieldDefinition[] = [
  { key: "entry", label: "条目名" },
  { key: "image_path", label: "文件路径" },
  { key: "launch_string", label: "启动命令" },
  { key: "location", label: "注册表位置" },
  { key: "publisher", label: "发布者" },
  { key: "description", label: "描述" },
];

export const NETWORK_FIELDS: FieldDefinition[] = [
  { key: "remote.addr", label: "远程 IP" },
  { key: "remote.port", label: "远程端口" },
  { key: "process_name", label: "进程名" },
  { key: "process_path", label: "进程路径" },
];

export const EVENT_FIELDS: FieldDefinition[] = [
  { key: "query_name", label: "DNS 查询域名" },
  { key: "destination_ip", label: "目标 IP" },
  { key: "process_path", label: "进程路径" },
  { key: "event_type", label: "事件类型" },
];

export function getFieldsForTarget(target: RuleTarget): FieldDefinition[] {
  switch (target) {
    case "Autorun": return AUTORUN_FIELDS;
    case "Network": return NETWORK_FIELDS;
    case "Event": return EVENT_FIELDS;
  }
}

// --- Association ---

export interface AssociationResult {
  sourceTab: WorkspaceTab;
  sourceKey: string;
  autoruns: AutorunItem[];
  network: NetConn[];
  events: SysmonEvent[];
}
