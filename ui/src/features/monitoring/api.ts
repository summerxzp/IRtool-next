import { invoke } from "@tauri-apps/api/core";

export interface RuntimeTelemetry {
  mode: { Foreground: null } | { Background: null };
  started_at: number | null;
  events_written: number;
  events_dropped: number;
  last_event_at: number | null;
  last_error: string | null;
}

export interface MonitorRule {
  id: string;
  name: string;
  targets: string[];
  event_types: string[];
  enabled: boolean;
}

export interface NotifyConfig {
  popup_rule_ids: string[];
  feishu_rule_ids: string[];
  feishu_webhook_url: string;
  popup_duration_secs: number;
}

export interface MonitorConfig {
  background_mode: boolean;
  persist_event_types: string[];
  retention_days: number;
  rules: MonitorRule[];
  db_path: string;
  enable_sni: boolean;
  enable_dns_pcap: boolean;
  adapter_ip: string | null;
  max_duration_secs: number;
  load_limit: number;
  max_size_mb: number;
  notify_config: NotifyConfig;
}

export async function getMonitorConfig(): Promise<MonitorConfig> {
  return invoke("cmd_monitor_get_config");
}

export async function updateMonitorConfig(config: MonitorConfig): Promise<void> {
  return invoke("cmd_monitor_update_config", { config });
}

export async function enterBackground(): Promise<void> {
  return invoke("cmd_monitor_enter_background");
}

export async function exitBackground(): Promise<void> {
  return invoke("cmd_monitor_exit_background");
}

export async function isBackground(): Promise<boolean> {
  return invoke("cmd_monitor_is_background");
}

export async function getTelemetry(): Promise<RuntimeTelemetry> {
  return invoke("cmd_monitor_get_telemetry");
}

export async function getEventCount(): Promise<number> {
  return invoke("cmd_monitor_get_event_count");
}
