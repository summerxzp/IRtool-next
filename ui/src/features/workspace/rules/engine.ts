import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";
import type { Condition, Rule, RuleTarget } from "../types";

// --- Condition evaluation ---

export function matchCondition(value: unknown, condition: Condition): boolean {
  const str = value == null ? "" : String(value);
  switch (condition.type) {
    case "contains":
      return str.includes(condition.value);
    case "regex": {
      try {
        return new RegExp(condition.value, "i").test(str);
      } catch {
        return false;
      }
    }
    case "equals":
      return str === condition.value;
  }
}

// --- Field accessors ---

export function getAutorunField(item: AutorunItem, field: string): unknown {
  switch (field) {
    case "entry": return item.entry;
    case "image_path": return item.image_path;
    case "launch_string": return item.launch_string;
    case "location": return item.location;
    case "publisher": return item.publisher;
    case "description": return item.description;
    case "category": return item.category;
    case "enabled": return item.enabled;
    case "timestamp": return item.timestamp;
    case "file_exists": return item.file_exists;
    case "file_size": return item.file_size;
    case "file_version": return item.file_version;
    case "service_name": return item.service_name;
    case "md5": return item.md5;
    case "sha256": return item.sha256;
    case "risk": return item.risk;
    case "risk_reasons": return item.risk_reasons;
    case "signature": return item.signature;
    default: return undefined;
  }
}

export function getNetworkField(item: NetConn, field: string): unknown {
  switch (field) {
    case "proto": return item.proto;
    case "family": return item.family;
    case "local.addr": return item.local.addr;
    case "local.port": return item.local.port;
    case "remote.addr": return item.remote.addr;
    case "remote.port": return item.remote.port;
    case "state": return item.state;
    case "pid": return item.pid;
    case "process_name": return item.process_name;
    case "process_path": return item.process_path;
    case "process_cmdline": return item.process_cmdline;
    case "first_seen": return item.first_seen;
    case "last_seen": return item.last_seen;
    case "is_current": return item.is_current;
    default: return undefined;
  }
}

export function getEventField(item: SysmonEvent, field: string): unknown {
  switch (field) {
    case "event_id": return item.event_id;
    case "event_type": return item.event_type;
    case "timestamp": return item.timestamp;
    case "timestamp_epoch": return item.timestamp_epoch;
    case "record_id": return item.record_id;
    case "raw_data": return item.raw_data;
    case "process_id": return item.process_id;
    case "process_name": return item.process_name;
    case "process_path": return item.process_path;
    case "user": return item.user;
    case "rule_name": return item.rule_name;
    case "query_name": return item.query_name;
    case "query_results": return item.query_results;
    case "query_status": return item.query_status;
    case "source_ip": return item.source_ip;
    case "source_port": return item.source_port;
    case "destination_ip": return item.destination_ip;
    case "destination_port": return item.destination_port;
    case "protocol": return item.protocol;
    case "initiated": return item.initiated;
    case "is_external": return item.is_external;
    case "source_process_id": return item.source_process_id;
    case "source_process_name": return item.source_process_name;
    case "source_process_path": return item.source_process_path;
    case "target_process_id": return item.target_process_id;
    case "target_process_name": return item.target_process_name;
    case "target_process_path": return item.target_process_path;
    case "start_address": return item.start_address;
    case "start_module": return item.start_module;
    case "start_function": return item.start_function;
    case "is_suspicious": return item.is_suspicious;
    case "target_filename": return item.target_filename;
    case "creation_utc_time": return item.creation_utc_time;
    default: return undefined;
  }
}

// --- Key generators ---

export function networkKey(item: NetConn): string {
  return `${item.proto}|${item.family}|${item.local.addr}:${item.local.port}|${item.remote.addr}:${item.remote.port}|${item.pid}`;
}

export function eventKey(item: SysmonEvent): string {
  return `${item.record_id ?? 0}-${item.timestamp}-${item.event_id}`;
}

// --- Rule scanning ---

function matchesRule(
  getField: (field: string) => unknown,
  rule: Rule,
): boolean {
  return rule.conditions.every((cond) => {
    const value = getField(cond.field);
    return matchCondition(value, cond);
  });
}

function filterRules(rules: Rule[], target: RuleTarget): Rule[] {
  return rules.filter((r) => r.enabled && r.target === target);
}

export function scanAutoruns(
  items: AutorunItem[],
  rules: Rule[],
): Map<number, Rule[]> {
  const targetRules = filterRules(rules, "Autorun");
  const result = new Map<number, Rule[]>();
  for (const item of items) {
    const getField = (field: string) => getAutorunField(item, field);
    const matched = targetRules.filter((r) => matchesRule(getField, r));
    if (matched.length > 0) {
      result.set(item.id, matched);
    }
  }
  return result;
}

export function scanNetwork(
  items: NetConn[],
  rules: Rule[],
): Map<string, Rule[]> {
  const targetRules = filterRules(rules, "Network");
  const result = new Map<string, Rule[]>();
  for (const item of items) {
    const getField = (field: string) => getNetworkField(item, field);
    const matched = targetRules.filter((r) => matchesRule(getField, r));
    if (matched.length > 0) {
      result.set(networkKey(item), matched);
    }
  }
  return result;
}

export function scanEvents(
  items: SysmonEvent[],
  rules: Rule[],
): Map<string, Rule[]> {
  const targetRules = filterRules(rules, "Event");
  const result = new Map<string, Rule[]>();
  for (const item of items) {
    const getField = (field: string) => getEventField(item, field);
    const matched = targetRules.filter((r) => matchesRule(getField, r));
    if (matched.length > 0) {
      result.set(eventKey(item), matched);
    }
  }
  return result;
}

// --- Keyword search ---

export function searchAutoruns(
  items: AutorunItem[],
  query: string,
): Set<number> {
  const q = query.toLowerCase();
  const result = new Set<number>();
  for (const item of items) {
    const haystack = [
      item.entry,
      item.image_path,
      item.launch_string,
      item.location,
      item.publisher,
      item.description,
      item.category,
      item.service_name,
      item.md5,
      item.sha256,
    ]
      .filter((v): v is string => v != null)
      .join("\n")
      .toLowerCase();
    if (haystack.includes(q)) {
      result.add(item.id);
    }
  }
  return result;
}

export function searchNetwork(
  items: NetConn[],
  query: string,
): Set<string> {
  const q = query.toLowerCase();
  const result = new Set<string>();
  for (const item of items) {
    const haystack = [
      item.proto,
      item.family,
      item.local.addr,
      String(item.local.port),
      item.remote.addr,
      String(item.remote.port),
      item.state,
      String(item.pid),
      item.process_name,
      item.process_path,
      item.process_cmdline,
    ]
      .filter((v): v is string => v != null)
      .join("\n")
      .toLowerCase();
    if (haystack.includes(q)) {
      result.add(networkKey(item));
    }
  }
  return result;
}

export function searchEvents(
  items: SysmonEvent[],
  query: string,
): Set<string> {
  const q = query.toLowerCase();
  const result = new Set<string>();
  for (const item of items) {
    const haystack = [
      String(item.event_id),
      item.event_type,
      item.timestamp,
      item.process_name,
      item.process_path,
      item.user,
      item.rule_name,
      item.query_name,
      item.query_results,
      item.source_ip,
      item.destination_ip,
      item.source_process_name,
      item.source_process_path,
      item.target_process_name,
      item.target_process_path,
      item.target_filename,
    ]
      .filter((v): v is string => v != null && v !== "")
      .join("\n")
      .toLowerCase();
    if (haystack.includes(q)) {
      result.add(eventKey(item));
    }
  }
  return result;
}
