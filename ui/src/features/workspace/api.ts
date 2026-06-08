import { commands } from "@/lib/bindings";
import { invoke } from "@tauri-apps/api/core";

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: any }): T {
  if (result.status === "error") throw result.error;
  return result.data;
}

/** Fetch autorun items from backend store */
export async function getAutorunItems() {
  return unwrap(await commands.cmdAutorunsGetResult());
}

/** Fetch network snapshot from backend store */
export async function getNetworkSnapshot() {
  return unwrap(await commands.cmdNetworkSnapshot());
}

/** Run a command template (attrib, takeown, 7z, etc.) */
export async function runCommand(program: string, args: string): Promise<string> {
  return await invoke<string>("cmd_workspace_run_command", { program, args });
}

// Re-export existing API for disposal operations
export { killProcess } from "@/features/network/api";
export { deleteEntry, openExplorer, openRegedit, openServices } from "@/features/autoruns/api";
