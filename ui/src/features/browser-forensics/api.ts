import { commands } from "@/lib/bindings";
import type { BrowserKind, BrowserProfile, ExtensionInventory, DownloadInfo, SessionRecoveryResult, HistoryAttribution, BrowserContext, RecentActivity } from "./types";

export async function listProfiles(): Promise<BrowserProfile[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsListProfiles();
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return [];
  }
}

export async function listExtensions(browser: BrowserKind, profileName: string): Promise<ExtensionInventory> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanExtensions(browser, profileName);
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return { browser, profile: profileName, extensions: [] };
  }
}

export async function scanAllExtensions(browser: BrowserKind): Promise<ExtensionInventory[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanAllExtensions(browser);
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return [];
  }
}

export async function listDownloads(browser: BrowserKind, profileName: string): Promise<DownloadInfo[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanDownloads(browser, profileName);
    if (result.status === "error") throw result.error;
    return result.data.downloads;
  } catch {
    return [];
  }
}

export async function recoverTabs(browser: BrowserKind, profile: string): Promise<SessionRecoveryResult> {
  try {
    const result = await (commands as any).cmdBrowserForensicsRecoverTabs(browser, profile);
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return { browser, profile, tabs: [], parse_errors: [] };
  }
}

export async function scanHistory(
  browser: BrowserKind,
  profileName: string,
): Promise<RecentActivity[]> {
  try {
    const result = await (commands as any).cmdBrowserForensicsScanHistory(browser, profileName);
    if (result.status === "error") throw result.error;
    return result.data.entries;
  } catch {
    return [];
  }
}

export async function getHistory(
  browser: BrowserKind,
  profileName: string,
  targetTime?: string,
): Promise<HistoryAttribution> {
  try {
    const result = await (commands as any).cmdBrowserForensicsAttributeHistory(
      browser,
      profileName,
      targetTime ?? new Date().toISOString(),
    );
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return { browser, profile: profileName, recent_browser_activity: [], navigation_chain: [] };
  }
}

export async function attributeBrowserContext(
  domain: string,
  ip: string | null,
  processName: string,
  pid: number,
  timestamp?: string,
): Promise<BrowserContext | null> {
  try {
    const result = await (commands as any).cmdBrowserForensicsContextAttribution(
      domain,
      ip,
      processName,
      pid,
      timestamp ?? new Date().toISOString(),
    );
    if (result.status === "error") throw result.error;
    return result.data;
  } catch {
    return null;
  }
}
