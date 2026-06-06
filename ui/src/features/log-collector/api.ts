import { commands, type IrError } from "@/lib/bindings";
import { invoke } from "@tauri-apps/api/core";
import type { SysmonEvent, SysmonStatus, EventConfigEntry } from "./types";

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: IrError }): T {
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function getStatus(): Promise<SysmonStatus> {
  return unwrap(await commands.cmdSysmonStatus());
}

export async function isChannelAvailable(): Promise<boolean> {
  return unwrap(await commands.cmdSysmonIsChannelAvailable());
}

export async function install(acceptEula: boolean): Promise<[boolean, string]> {
  return unwrap(await commands.cmdSysmonInstall(acceptEula));
}

export async function uninstall(): Promise<[boolean, string]> {
  return unwrap(await commands.cmdSysmonUninstall());
}

export async function updateConfig(): Promise<[boolean, string]> {
  return unwrap(await commands.cmdSysmonUpdateConfig());
}

export async function getExistingEvents(limit: number, enabledEventIds: number[]): Promise<SysmonEvent[]> {
  return unwrap(await commands.cmdSysmonGetExistingEvents(limit, enabledEventIds));
}

export async function getDefaultEventConfigs(): Promise<EventConfigEntry[]> {
  return unwrap(await commands.cmdSysmonDefaultEventConfigs());
}

export async function generateConfig(enabledEvents: string[]): Promise<string> {
  return unwrap(await commands.cmdSysmonGenerateConfig(enabledEvents));
}

// New commands not yet in bindings — use invoke directly
export async function startSubscription(enabledEventIds: number[], pollIntervalMs?: number): Promise<void> {
  await invoke("cmd_sysmon_start_subscription", {
    enabledEventIds,
    pollIntervalMs: pollIntervalMs ?? null,
  });
}

export async function stopSubscription(): Promise<void> {
  await invoke("cmd_sysmon_stop_subscription");
}

export async function isSubscribing(): Promise<boolean> {
  return await invoke("cmd_sysmon_is_subscribing");
}

export async function getLogMaxSize(): Promise<number> {
  return await invoke<number>("cmd_sysmon_get_log_max_size");
}

export async function setLogMaxSize(sizeMb: number): Promise<void> {
  await invoke("cmd_sysmon_set_log_max_size", { sizeMb });
}
