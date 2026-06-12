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

/** Run a command template (attrib, takeown, 7z, etc.) — debug builds only */
export async function runCommand(program: string, args: string): Promise<string> {
  return await invoke<string>("cmd_workspace_run_command", { program, args });
}

/** Unhide a file or directory (remove hidden attribute) */
export async function unhidePath(path: string): Promise<string> {
  return invoke<string>("cmd_workspace_unhide_path", { path });
}

/** Take ownership of a file or directory */
export async function takeOwnership(path: string): Promise<string> {
  return invoke<string>("cmd_workspace_take_ownership", { path });
}

/** Sample a file (zip with password protection) */
export async function samplePath(path: string, outputDir: string, password: string): Promise<string> {
  return invoke<string>("cmd_workspace_sample_path", { path, outputDir, password });
}

/** Open a path in explorer */
export async function openPath(path: string): Promise<string> {
  return invoke<string>("cmd_workspace_open_path", { path });
}

// Re-export existing API for disposal operations
export { killProcess } from "@/features/network/api";
export { deleteEntry, openExplorer, openRegedit, openServices } from "@/features/autoruns/api";
