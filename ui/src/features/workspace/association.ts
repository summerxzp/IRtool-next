import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";
import type { WorkspaceTab, AssociationResult } from "./types";

/**
 * Find associated records across data sources based on strict matching keys:
 * - PID (process ID)
 * - Process path (image_path / process_path)
 * - IP address (remote.addr / destination_ip) — only non-private IPs
 */
export function findAssociations(
  sourceTab: WorkspaceTab,
  sourceItem: AutorunItem | NetConn | SysmonEvent,
  autoruns: AutorunItem[],
  network: NetConn[],
  events: SysmonEvent[],
): AssociationResult {
  const matchKeys: string[] = [];
  const pids: number[] = [];
  const paths: string[] = [];
  const ips: string[] = [];

  // Extract matching keys from source item
  switch (sourceTab) {
    case "autoruns": {
      const item = sourceItem as AutorunItem;
      if (item.image_path) paths.push(item.image_path.toLowerCase());
      break;
    }
    case "network": {
      const item = sourceItem as NetConn;
      pids.push(item.pid);
      if (item.process_path) paths.push(item.process_path.toLowerCase());
      if (item.remote.addr && !isPrivateIp(item.remote.addr)) {
        ips.push(item.remote.addr);
      }
      break;
    }
    case "events": {
      const item = sourceItem as SysmonEvent;
      pids.push(item.process_id);
      if (item.process_path) paths.push(item.process_path.toLowerCase());
      if (item.destination_ip && !isPrivateIp(item.destination_ip)) {
        ips.push(item.destination_ip);
      }
      break;
    }
  }

  // Build match key description
  if (pids.length > 0) matchKeys.push(`PID: ${pids.join(", ")}`);
  if (paths.length > 0) matchKeys.push(`路径: ${paths.join(", ")}`);
  if (ips.length > 0) matchKeys.push(`IP: ${ips.join(", ")}`);

  // Find associated autorun items (by path only)
  const matchedAutoruns = autoruns.filter((item) => {
    if (paths.length > 0 && item.image_path) {
      if (paths.includes(item.image_path.toLowerCase())) return true;
    }
    return false;
  });

  // Find associated network items (by PID, path, or IP)
  const matchedNetwork = network.filter((item) => {
    if (pids.includes(item.pid)) return true;
    if (paths.length > 0 && item.process_path) {
      if (paths.includes(item.process_path.toLowerCase())) return true;
    }
    if (ips.length > 0 && ips.includes(item.remote.addr)) return true;
    return false;
  });

  // Find associated event items (by PID, path, or IP)
  const matchedEvents = events.filter((item) => {
    if (pids.includes(item.process_id)) return true;
    if (paths.length > 0 && item.process_path) {
      if (paths.includes(item.process_path.toLowerCase())) return true;
    }
    if (ips.length > 0 && ips.includes(item.destination_ip)) return true;
    return false;
  });

  return {
    sourceTab,
    sourceKey: matchKeys.join(" / "),
    autoruns: matchedAutoruns,
    network: matchedNetwork,
    events: matchedEvents,
  };
}

function isPrivateIp(ip: string): boolean {
  if (!ip || ip === "0.0.0.0" || ip === "::" || ip === "::ffff:0.0.0.0" || ip === "*") return true;
  const lower = ip.toLowerCase();
  if (lower.startsWith("10.") || lower.startsWith("192.168.") || lower.startsWith("127.") || lower === "::1") return true;
  if (lower.startsWith("172.")) {
    const parts = lower.split(".");
    if (parts.length >= 2) {
      const second = parseInt(parts[1], 10);
      if (second >= 16 && second <= 31) return true;
    }
  }
  if (lower.startsWith("169.254.") || lower.startsWith("fe80")) return true;
  return false;
}
