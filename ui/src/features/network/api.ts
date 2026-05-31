import { commands } from "@/lib/bindings";
import type {
  NetConn,
  NetworkPollingControl,
  NetworkSnapshotPayload,
} from "./types";

export async function snapshot(): Promise<NetworkSnapshotPayload> {
  return commands.cmdNetworkSnapshot();
}

export async function killProcess(pid: number): Promise<void> {
  return commands.cmdNetworkKillProcess(pid);
}

export async function setPolling(control: NetworkPollingControl): Promise<void> {
  return commands.cmdNetworkSetPolling(control);
}

export async function clearHistory(): Promise<void> {
  return commands.cmdNetworkClearHistory();
}

export type { NetConn };
