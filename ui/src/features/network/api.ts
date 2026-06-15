import { commands } from "@/lib/bindings";
import type { NetConn, NetworkPollingControl } from "./types";

export async function snapshot() {
  const result = await commands.cmdNetworkSnapshot();
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function killProcess(pid: number) {
  const result = await commands.cmdNetworkKillProcess(pid);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function setPolling(control: NetworkPollingControl) {
  const result = await commands.cmdNetworkSetPolling(control);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function clearHistory() {
  const result = await commands.cmdNetworkClearHistory();
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function refreshCmdline(pid: number) {
  const result = await commands.cmdNetworkRefreshCmdline(pid);
  if (result.status === "error") throw result.error;
  return result.data;
}

export type { NetConn };
