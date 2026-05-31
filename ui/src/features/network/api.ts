import { invoke } from "@tauri-apps/api/core";
import type {
  NetConn,
  NetworkPollingControl,
  NetworkSnapshotPayload,
} from "./types";

export async function snapshot(): Promise<NetworkSnapshotPayload> {
  return invoke("cmd_network_snapshot");
}

export async function killProcess(pid: number): Promise<void> {
  return invoke("cmd_network_kill_process", { pid });
}

export async function setPolling(control: NetworkPollingControl): Promise<void> {
  return invoke("cmd_network_set_polling", { control });
}

export async function clearHistory(): Promise<void> {
  return invoke("cmd_network_clear_history");
}

export type { NetConn };
