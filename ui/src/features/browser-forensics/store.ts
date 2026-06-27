import { create } from "zustand";
import type {
  BrowserKind, BrowserProfile, ExtensionInfo, DownloadInfo, RecoveredTab,
  HistoryEntry, HistoryAttribution, EvidenceObject, ExtensionAttributionPayload, DomainAttribution,
} from "./types";

export type ForensicsTab = "extensions" | "history" | "downloads" | "tabs" | "context";

// ── P1.3: Helper Extension 归因融合辅助函数 ──────────────────────

/// 从 URL 提取 hostname，失败返回 null
export function urlToHostname(url: string): string | null {
  try {
    return new URL(url).hostname;
  } catch {
    return null;
  }
}

/// 判断 hostname 是否匹配 target domain（精确匹配或子域名后缀匹配）
/// 例如 target="evil.com" 匹配 hostname="evil.com" 或 "sub.evil.com"，不匹配 "notevil.com"
export function hostnameMatchesDomain(hostname: string, target: string): boolean {
  if (hostname === target) return true;
  if (hostname.endsWith("." + target)) return true;
  return false;
}

/// 根据已累积的扩展归因事件，判断是否应升级 EvidenceObject.extension_attribution.confidence 为 confirmed
/// 条件：存在 level="confirmed" 且 url hostname 匹配 target domain 的事件
export function shouldUpgradeToConfirmed(
  events: ExtensionAttributionPayload[],
  targetDomain: string,
): boolean {
  return events.some((evt) => {
    if (evt.level !== "confirmed") return false;
    const hostname = urlToHostname(evt.url);
    if (!hostname) return false;
    return hostnameMatchesDomain(hostname, targetDomain);
  });
}

interface BrowserForensicsState {
  selectedBrowser: BrowserKind;
  setSelectedBrowser: (b: BrowserKind) => void;

  selectedProfile: string | null;
  setSelectedProfile: (p: string | null) => void;

  profiles: BrowserProfile[];
  setProfiles: (p: BrowserProfile[]) => void;

  activeTab: ForensicsTab;
  setActiveTab: (t: ForensicsTab) => void;

  extensions: ExtensionInfo[];
  setExtensions: (e: ExtensionInfo[]) => void;

  downloads: DownloadInfo[];
  setDownloads: (d: DownloadInfo[]) => void;

  tabs: RecoveredTab[];
  setTabs: (t: RecoveredTab[]) => void;

  history: HistoryEntry[];
  setHistory: (h: HistoryEntry[]) => void;

  historySince: string; // "1h" | "24h" | "7d" | "all"
  setHistorySince: (s: string) => void;

  // Context Attribution state
  contextResult: EvidenceObject | null;
  setContextResult: (c: EvidenceObject | null) => void;

  // Domain Attribution state
  domainAttribution: DomainAttribution[] | null;
  setDomainAttribution: (d: DomainAttribution[] | null) => void;

  contextLoading: boolean;
  setContextLoading: (v: boolean) => void;

  loading: boolean;
  setLoading: (v: boolean) => void;

  error: string | null;
  setError: (e: string | null) => void;

  selectedExtensionId: string | null;
  setSelectedExtensionId: (id: string | null) => void;

  search: string;
  setSearch: (s: string) => void;

  // 临时监控：关注的目标域名/IP列表（下发到 Helper Extension）
  watchTargets: string[];
  addWatchTarget: (target: string) => void;
  removeWatchTarget: (target: string) => void;
  clearWatchTargets: () => void;
  /// 批量设置 watchTargets（用于启动时从磁盘 config 同步）
  setWatchTargets: (targets: string[]) => void;

  // History Attribution state
  historyAttribution: HistoryAttribution | null;
  setHistoryAttribution: (h: HistoryAttribution | null) => void;

  // 实时扩展归因事件（来自 Helper Extension）
  extensionAttributions: ExtensionAttributionPayload[];
  addExtensionAttribution: (e: ExtensionAttributionPayload) => void;
  clearExtensionAttributions: () => void;

  /// 当 Helper Extension confirmed 事件到达时，升级当前 contextResult 的 extension_attribution.confidence
  upgradeContextExtensionConfidence: (domain: string) => void;
}

export const useBrowserForensicsStore = create<BrowserForensicsState>()((set) => ({
  selectedBrowser: "chrome",
  setSelectedBrowser: (selectedBrowser) => set({ selectedBrowser }),

  selectedProfile: null,
  setSelectedProfile: (selectedProfile) => set({ selectedProfile }),

  profiles: [],
  setProfiles: (profiles) => set({ profiles }),

  activeTab: "extensions",
  setActiveTab: (activeTab) => set({ activeTab }),

  extensions: [],
  setExtensions: (extensions) => set({ extensions }),

  downloads: [],
  setDownloads: (downloads) => set({ downloads }),

  tabs: [],
  setTabs: (tabs) => set({ tabs }),

  history: [],
  setHistory: (history) => set({ history }),

  historySince: "all",
  setHistorySince: (historySince) => set({ historySince }),

  contextResult: null,
  setContextResult: (contextResult) => set({ contextResult }),

  domainAttribution: null,
  setDomainAttribution: (domainAttribution) => set({ domainAttribution }),

  contextLoading: false,
  setContextLoading: (contextLoading) => set({ contextLoading }),

  loading: false,
  setLoading: (loading) => set({ loading }),

  error: null,
  setError: (error) => set({ error }),

  selectedExtensionId: null,
  setSelectedExtensionId: (selectedExtensionId) => set({ selectedExtensionId }),

  search: "",
  setSearch: (search) => set({ search }),

  watchTargets: [],
  addWatchTarget: (target) =>
    set((state) => {
      const trimmed = target.trim();
      if (!trimmed) return state;
      if (state.watchTargets.includes(trimmed)) return state;
      return { watchTargets: [...state.watchTargets, trimmed] };
    }),
  removeWatchTarget: (target) =>
    set((state) => ({
      watchTargets: state.watchTargets.filter((t) => t !== target),
    })),
  clearWatchTargets: () => set({ watchTargets: [] }),
  setWatchTargets: (targets) =>
    set(() => ({
      // 去重 + trim，保持插入顺序
      watchTargets: Array.from(
        new Set(targets.map((t) => t.trim()).filter(Boolean)),
      ),
    })),

  extensionAttributions: [],
  addExtensionAttribution: (e) =>
    set((state) => ({
      extensionAttributions: [...state.extensionAttributions, e].slice(-200),
    })),
  clearExtensionAttributions: () => set({ extensionAttributions: [] }),

  upgradeContextExtensionConfidence: (domain) =>
    set((state) => {
      const ctx = state.contextResult;
      if (!ctx || ctx.domain !== domain) return state;
      if (!ctx.extension_attribution) return state;
      if (ctx.extension_attribution.confidence === "confirmed") return state;
      return {
        contextResult: {
          ...ctx,
          extension_attribution: {
            ...ctx.extension_attribution,
            confidence: "confirmed" as const,
          },
        },
      };
    }),

  historyAttribution: null,
  setHistoryAttribution: (historyAttribution) => set({ historyAttribution }),
}));
