import { commands } from "@/lib/bindings";
import type { ExtensionConnectionStatus, ReconnectDiagnostics, CdpCaptureStatus } from "@/lib/bindings";
import type { BrowserKind, BrowserProfile, ExtensionInventory, DownloadInfo, HistoryAttribution, EvidenceObject, HistoryEntry, ExtensionAttribution, DomainAttribution } from "./types";

export type { ExtensionConnectionStatus, ReconnectDiagnostics, CdpCaptureStatus };

/// 将后端 IrError（tagged union）转为抛出的 Error。
///
/// 与 crates/irtool-core/src/error.rs 的 `#[error(...)]` Display 实现保持一致。
export function throwIrError(err: unknown): never {
  if (err && typeof err === "object" && "kind" in err) {
    const { kind, message } = err as { kind: string; message?: unknown };
    switch (kind) {
      case "permission_denied":
        throw new Error("permission denied: requires administrator");
      case "cancelled":
        throw new Error("cancelled");
      case "external_tool": {
        const m = message as { tool?: string; code?: number } | undefined;
        throw new Error(`external tool failed: ${m?.tool} exit=${m?.code}`);
      }
      default:
        throw new Error(`${kind}: ${message}`);
    }
  }
  throw new Error(String(err));
}

export async function listProfiles(onError?: (msg: string) => void): Promise<BrowserProfile[]> {
  try {
    const result = await commands.cmdBrowserForensicsListProfiles();
    if (result.status === "error") throwIrError(result.error);
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
    const result = await commands.cmdBrowserForensicsScanExtensions(browser, profileName);
    if (result.status === "error") throwIrError(result.error);
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
    const result = await commands.cmdBrowserForensicsScanAllExtensions(browser);
    if (result.status === "error") throwIrError(result.error);
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
    const result = await commands.cmdBrowserForensicsScanDownloads(browser, profileName);
    if (result.status === "error") throwIrError(result.error);
    return result.data.downloads;
  } catch (e) {
    const msg = `Failed to list downloads: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

export async function scanHistory(
  browser: BrowserKind,
  profileName: string,
  since?: number,
  onError?: (msg: string) => void,
): Promise<HistoryEntry[]> {
  try {
    const result = await commands.cmdBrowserForensicsScanHistory(browser, profileName, null, since ?? null);
    if (result.status === "error") throwIrError(result.error);
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
    const result = await commands.cmdBrowserForensicsAttributeHistory(
      browser,
      profileName,
      targetTime ?? new Date().toISOString(),
    );
    if (result.status === "error") throwIrError(result.error);
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
): Promise<EvidenceObject | null> {
  try {
    const result = await commands.cmdBrowserForensicsContextAttribution(
      domain,
      ip,
      processName,
      pid,
      timestamp ?? new Date().toISOString(),
      cmdline ?? null,
    );
    if (result.status === "error") throwIrError(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to attribute browser context: ${e}`;
    console.error(msg);
    onError?.(msg);
    return null;
  }
}

export async function attributeExtension(
  processName: string,
  pid: number,
  domain: string,
  cmdline?: string,
  onError?: (msg: string) => void,
): Promise<ExtensionAttribution | null> {
  try {
    const result = await commands.cmdBrowserForensicsAttributeExtension(
      processName,
      pid,
      domain,
      cmdline ?? null,
    );
    if (result.status === "error") throwIrError(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to attribute extension: ${e}`;
    console.error(msg);
    onError?.(msg);
    return null;
  }
}

export async function attributeByDomain(
  target: string,
  browser: BrowserKind,
  onError?: (msg: string) => void,
): Promise<DomainAttribution[]> {
  try {
    const result = await commands.cmdBrowserForensicsAttributeByDomain(target, browser);
    if (result.status === "error") throwIrError(result.error);
    return result.data;
  } catch (e) {
    const msg = `Failed to attribute by domain: ${e}`;
    console.error(msg);
    onError?.(msg);
    return [];
  }
}

/// 下发 filterDomains 到 Helper Extension（通过 Native Messaging）
/// 空数组 → 清除过滤（全量上报）
export async function sendConfig(
  filterDomains: string[],
  onError?: (msg: string) => void,
): Promise<void> {
  try {
    const result = await commands.cmdBrowserForensicsSendConfig(filterDomains);
    if (result.status === "error") throwIrError(result.error);
  } catch (e) {
    const msg = `Failed to send config: ${e}`;
    console.error(msg);
    onError?.(msg);
  }
}

/// 读取已下发的 filterDomains（启动时同步 UI 与磁盘 config 用）
/// 返回空数组表示：文件不存在/字段缺失/解析失败，或确实没下发过
export async function getNativeConfig(): Promise<string[]> {
  try {
    const result = await commands.cmdBrowserForensicsGetConfig();
    if (result.status === "error") throwIrError(result.error);
    return result.data ?? [];
  } catch (e) {
    console.error("Failed to get native config:", e);
    return [];
  }
}

/// 下发自我卸载信号给扩展（手动清理，立即卸载扩展）
export async function selfUninstall(): Promise<void> {
  const result = await commands.cmdBrowserForensicsSelfUninstall();
  if (result.status === "error") throwIrError(result.error);
}

/// 设置扩展自我清理超时时间（分钟）
/// 0 = 禁用自动清理，>0 = 启用
export async function setSelfCleanupTimeout(timeoutMin: number): Promise<void> {
  const result = await commands.cmdBrowserForensicsSetSelfCleanupTimeout(timeoutMin);
  if (result.status === "error") throwIrError(result.error);
}

/// 读取当前 selfCleanupTimeoutMin 配置
/// 返回 null 表示文件不存在或字段缺失，UI 应显示默认值 60
export async function getSelfCleanupTimeout(): Promise<number | null> {
  try {
    const result = await commands.cmdBrowserForensicsGetSelfCleanupTimeout();
    if (result.status === "error") throwIrError(result.error);
    return result.data;
  } catch (e) {
    console.error("Failed to get self-cleanup timeout:", e);
    return null;
  }
}

/// 安装 Native Messaging Host（bindings 已有，re-export 统一入口）
///
/// 扩展 ID 默认由 manifest.json 的 `key` 字段固定，无需前端传入。
/// `extensionIdOverride` 用于兜底场景（高级选项），用户可手动输入扩展 ID 覆盖。
export async function installNativeMessagingHost(
  browser: BrowserKind,
  extensionIdOverride?: string,
): Promise<string> {
  const result = await commands.cmdBrowserForensicsInstallNativeMessagingHost(
    browser,
    extensionIdOverride ?? null,
  );
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 获取 Helper Extension 目录绝对路径（新命令，bindings 可能尚未生成）
export async function getHelperExtensionPath(): Promise<string> {
  const result = await commands.cmdBrowserForensicsGetHelperExtensionPath();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 打开浏览器扩展管理页（新命令，bindings 可能尚未生成）
export async function openExtensionsPage(browser: BrowserKind): Promise<void> {
  const result = await commands.cmdBrowserForensicsOpenExtensionsPage(browser);
  if (result.status === "error") throwIrError(result.error);
}

/// 查询 Helper Extension 连接状态
export async function getExtensionStatus(): Promise<ExtensionConnectionStatus> {
  const result = await commands.cmdBrowserForensicsExtensionStatus();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 重新连接 Helper Extension（kill NMH 进程 + 返回诊断信息）
export async function reconnectExtension(): Promise<ReconnectDiagnostics> {
  const result = await commands.cmdBrowserForensicsReconnectExtension();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

// ── CDP 远程调试抓包 ──────────────────────────────────────────

/// 探测浏览器调试端口（不启动抓包服务）。
/// 返回 null 表示无浏览器开启调试端口。
export async function cdpProbe(): Promise<CdpCaptureStatus | null> {
  const result = await commands.cmdBrowserForensicsCdpProbe();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 启动 CDP 抓包服务。
/// 失败会抛出错误（如端口未开启）。
export async function cdpCaptureStart(): Promise<CdpCaptureStatus> {
  const result = await commands.cmdBrowserForensicsCdpCaptureStart();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 停止 CDP 抓包服务。
export async function cdpCaptureStop(): Promise<void> {
  const result = await commands.cmdBrowserForensicsCdpCaptureStop();
  if (result.status === "error") throwIrError(result.error);
}

/// 查询 CDP 抓包服务状态。
export async function cdpCaptureStatus(): Promise<CdpCaptureStatus> {
  const result = await commands.cmdBrowserForensicsCdpCaptureStatus();
  if (result.status === "error") throwIrError(result.error);
  return result.data;
}

/// 启动带调试端口的浏览器（独立临时 profile）。
/// 用于"一键启动调试浏览器"按钮。
export async function launchBrowserWithDebugPort(browser: BrowserKind): Promise<void> {
  const result = await commands.cmdBrowserForensicsLaunchBrowserWithDebugPort(browser);
  if (result.status === "error") throwIrError(result.error);
}

