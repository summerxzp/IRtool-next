import { commands } from "@/lib/bindings";
import type { BrowserKind, BrowserProfile, ExtensionInventory, DownloadInfo, SessionRecoveryResult, HistoryAttribution, BrowserContext, HistoryEntry } from "./types";

export async function listProfiles(onError?: (msg: string) => void): Promise<BrowserProfile[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsListProfiles();
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to list profiles: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

export async function listExtensions(browser: BrowserKind, profileName: string, onError?: (msg: string) => void): Promise<ExtensionInventory> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanExtensions(browser, profileName);
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to list extensions: ${e}`;
    console.error(msg);
    onError?.(msg);
    return { browser, profile: profileName, extensions: [] };
  }
}

export async function scanAllExtensions(browser: BrowserKind, onError?: (msg: string) => void): Promise<ExtensionInventory[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanAllExtensions(browser);
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to scan all extensions: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

export async function listDownloads(browser: BrowserKind, profileName: string, onError?: (msg: string) => void): Promise<DownloadInfo[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanDownloads(browser, profileName);
    if (result.status === "error") throw new Error(result.error);
    return result.data.downloads;
  } catch (e) {
    const msg = `Failed to list downloads: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

export async function recoverTabs(browser: BrowserKind, profile: string, onError?: (msg: string) => void): Promise<SessionRecoveryResult> {
  try {
    const result = await (commands as any).cmdBrowserForensicsRecoverTabs(browser, profile);
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to recover tabs: ${e}`;
    console.error(msg);
    onError?.(msg);
    return { browser, profile, tabs: [], parse_errors: [] };
  }
}

export async function scanHistory(
  browser: BrowserKind,
  profileName: string,
  onError?: (msg: string) => void,
): Promise<HistoryEntry[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanHistory(browser, profileName);
    if (result.status === "error") throw new Error(result.error);
    return result.data.entries;
  } catch (e) {
    const msg = `Failed to scan history: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

export async function getHistory(
  browser: BrowserKind,
  profileName: string,
  targetTime?: string,
  onError?: (msg: string) => void,
): Promise<HistoryAttribution> {
  try {
    const result = await (commands as any).cmdBrowserForensicsAttributeHistory(
      browser,
      profileName,
      targetTime ?? new Date().toISOString(),
    );
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to get history: ${e}`;
    console.error(msg);
    onError?.(msg);
    return { browser, profile: profileName, recent_browser_activity: [], navigation_chain: [] };
  }
}

export async function attributeBrowserContext(
  domain: string,
  ip: string | null,
  processName: string,
  pid: number,
  cmdline?: string,
  timestamp?: string,
  onError?: (msg: string) => void,
): Promise<BrowserContext | null> {
  try {
    const result = await (commands as any).cmdBrowserForensicsContextAttribution(
      domain,
      ip,
      processName,
      pid,
      cmdline ?? null,
      timestamp ?? new Date().toISOString(),
    );
    if (result.status === "error") throw new Error(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to attribute browser context: ${e}`;
    console.error(msg);
    onError?.(msg);
    return null;
  }
}
