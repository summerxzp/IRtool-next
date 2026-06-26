import { create } from "zustand";
import type {
  BrowserKind, BrowserProfile, ExtensionInfo, DownloadInfo, RecoveredTab,
  HistoryEntry, HistoryAttribution, BrowserContext, ExtensionAttributionPayload, DomainAttribution,
} from "./types";

export type ForensicsTab = "extensions" | "history" | "downloads" | "tabs" | "context";

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
  contextResult: BrowserContext | null;
  setContextResult: (c: BrowserContext | null) => void;

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

  // History Attribution state
  historyAttribution: HistoryAttribution | null;
  setHistoryAttribution: (h: HistoryAttribution | null) => void;

  // 实时扩展归因事件（来自 Helper Extension）
  extensionAttributions: ExtensionAttributionPayload[];
  addExtensionAttribution: (e: ExtensionAttributionPayload) => void;
  clearExtensionAttributions: () => void;
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

  extensionAttributions: [],
  addExtensionAttribution: (e) =>
    set((state) => ({
      extensionAttributions: [...state.extensionAttributions, e].slice(-200),
    })),
  clearExtensionAttributions: () => set({ extensionAttributions: [] }),

  historyAttribution: null,
  setHistoryAttribution: (historyAttribution) => set({ historyAttribution }),
}));
