import { create } from "zustand";
import type {
  BrowserKind, BrowserProfile, ExtensionInfo, DownloadInfo, RecoveredTab,
  RecentActivity, BrowserContext,
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

  history: RecentActivity[];
  setHistory: (h: RecentActivity[]) => void;

  // Context Attribution state
  contextResult: BrowserContext | null;
  setContextResult: (c: BrowserContext | null) => void;

  contextInputDomain: string;
  setContextInputDomain: (d: string) => void;

  contextInputPid: string;
  setContextInputPid: (p: string) => void;

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

  contextResult: null,
  setContextResult: (contextResult) => set({ contextResult }),

  contextInputDomain: "",
  setContextInputDomain: (contextInputDomain) => set({ contextInputDomain }),

  contextInputPid: "",
  setContextInputPid: (contextInputPid) => set({ contextInputPid }),

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
}));
