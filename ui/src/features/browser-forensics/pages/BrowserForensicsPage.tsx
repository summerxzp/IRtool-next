import { useEffect, useMemo, useRef, useCallback } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { ScanLine } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useBrowserForensicsStore, type ForensicsTab } from "../store";
import * as api from "../api";
import { ExtensionTable } from "../components/ExtensionTable";
import { ExtensionDetail } from "../components/ExtensionDetail";
import { HistoryTable } from "../components/HistoryTable";
import { DownloadTable } from "../components/DownloadTable";
import { TabTable } from "../components/TabTable";
import { ContextAttributionPanel } from "../components/ContextAttributionPanel";
import type { BrowserKind, ExtensionInfo, BrowserMaliciousConnectionPayload } from "../types";

const BROWSERS: { kind: BrowserKind; label: string }[] = [
  { kind: "chrome", label: "Chrome" },
  { kind: "edge", label: "Edge" },
  { kind: "brave", label: "Brave" },
  { kind: "vivaldi", label: "Vivaldi" },
];

const TAB_KEYS: { key: ForensicsTab; i18nKey: string }[] = [
  { key: "extensions", i18nKey: "browser-forensics.extensions" },
  { key: "history", i18nKey: "browser-forensics.history" },
  { key: "downloads", i18nKey: "browser-forensics.downloads" },
  { key: "tabs", i18nKey: "browser-forensics.tabs" },
  { key: "context", i18nKey: "browser-forensics.context-attribution" },
];

export function BrowserForensicsPage() {
  const { t } = useTranslation();

  const selectedBrowser = useBrowserForensicsStore((s) => s.selectedBrowser);
  const setSelectedBrowser = useBrowserForensicsStore((s) => s.setSelectedBrowser);
  const selectedProfile = useBrowserForensicsStore((s) => s.selectedProfile);
  const setSelectedProfile = useBrowserForensicsStore((s) => s.setSelectedProfile);
  const profiles = useBrowserForensicsStore((s) => s.profiles);
  const setProfiles = useBrowserForensicsStore((s) => s.setProfiles);
  const activeTab = useBrowserForensicsStore((s) => s.activeTab);
  const setActiveTab = useBrowserForensicsStore((s) => s.setActiveTab);
  const extensions = useBrowserForensicsStore((s) => s.extensions);
  const setExtensions = useBrowserForensicsStore((s) => s.setExtensions);
  const downloads = useBrowserForensicsStore((s) => s.downloads);
  const setDownloads = useBrowserForensicsStore((s) => s.setDownloads);
  const tabs = useBrowserForensicsStore((s) => s.tabs);
  const setTabs = useBrowserForensicsStore((s) => s.setTabs);
  const history = useBrowserForensicsStore((s) => s.history);
  const setHistory = useBrowserForensicsStore((s) => s.setHistory);
  const loading = useBrowserForensicsStore((s) => s.loading);
  const setLoading = useBrowserForensicsStore((s) => s.setLoading);
  const error = useBrowserForensicsStore((s) => s.error);
  const setError = useBrowserForensicsStore((s) => s.setError);
  const selectedExtensionId = useBrowserForensicsStore((s) => s.selectedExtensionId);
  const setSelectedExtensionId = useBrowserForensicsStore((s) => s.setSelectedExtensionId);
  const search = useBrowserForensicsStore((s) => s.search);
  const setSearch = useBrowserForensicsStore((s) => s.setSearch);
  const setContextInputDomain = useBrowserForensicsStore((s) => s.setContextInputDomain);
  const setContextInputPid = useBrowserForensicsStore((s) => s.setContextInputPid);

  // Load profiles on mount (auto — no privacy concern, just file listing)
  useEffect(() => {
    api.listProfiles().then(setProfiles);
  }, [setProfiles]);

  // Auto-select first profile on mount
  useEffect(() => {
    if (profiles.length > 0 && !selectedProfile) {
      setSelectedProfile(profiles[0].name);
    }
  }, [profiles, selectedProfile, setSelectedProfile]);

  // ── 监听后端推送的浏览器恶意连接事件 ──────────────────
  useEffect(() => {
    const unlistenPromise = listen<BrowserMaliciousConnectionPayload>(
      "evt_browser_malicious_connection",
      (event) => {
        const { domain, pid } = event.payload;
        console.info("[browser-forensics] malicious connection detected:", event.payload);

        // 预填 Context Attribution 查询参数并自动切换到 context 面板
        setContextInputDomain(domain);
        setContextInputPid(String(pid));
        setActiveTab("context");
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── 扫描：按需触发，替代自动加载 ──────────────────────
  const reqIdRef = useRef(0);
  const runScan = useCallback(() => {
    if (!selectedProfile) return;
    const reqId = ++reqIdRef.current;

    setExtensions([]);
    setDownloads([]);
    setTabs([]);
    setHistory([]);
    setSelectedExtensionId(null);
    setLoading(true);
    setError(null);

    const isCurrent = () => reqId === reqIdRef.current;

    const fetches: Promise<void>[] = [];

    fetches.push(
      api.listExtensions(selectedBrowser, selectedProfile).then((inv) => {
        if (isCurrent()) setExtensions(inv.extensions);
      }).catch((e) => { if (isCurrent()) setError(String(e)); }),
    );
    fetches.push(
      api.listDownloads(selectedBrowser, selectedProfile).then((d) => {
        if (isCurrent()) setDownloads(d);
      }).catch((e) => { if (isCurrent()) setError(String(e)); }),
    );
    fetches.push(
      api.recoverTabs(selectedBrowser, selectedProfile).then((r) => {
        if (isCurrent()) setTabs(r.tabs);
      }).catch((e) => { if (isCurrent()) setError(String(e)); }),
    );
    fetches.push(
      api.scanHistory(selectedBrowser, selectedProfile).then((h) => {
        if (isCurrent()) setHistory(h);
      }).catch((e) => { if (isCurrent()) setError(String(e)); }),
    );

    Promise.all(fetches).finally(() => {
      if (isCurrent()) setLoading(false);
    });
  }, [selectedBrowser, selectedProfile]);

  const selectedExtension = useMemo(
    () => extensions.find((e) => e.id === selectedExtensionId) ?? null,
    [extensions, selectedExtensionId],
  );

  const handleExtensionSelect = (row: ExtensionInfo | null) => {
    setSelectedExtensionId(row?.id ?? null);
  };

  // 切换浏览器：重置 profile + 清空数据（不自动扫描）
  const handleBrowserSwitch = useCallback((kind: BrowserKind) => {
    if (kind === selectedBrowser) return;
    setSelectedBrowser(kind);
    const firstProfile = profiles.find((p) => p.browser === kind);
    setSelectedProfile(firstProfile?.name ?? null);
    setExtensions([]);
    setDownloads([]);
    setTabs([]);
    setHistory([]);
    setSelectedExtensionId(null);
    setError(null);
  }, [selectedBrowser, profiles]);

  // 切换 profile：清空数据（不自动扫描）
  const handleProfileChange = useCallback((value: string) => {
    setSelectedProfile(value);
    setExtensions([]);
    setDownloads([]);
    setTabs([]);
    setHistory([]);
    setSelectedExtensionId(null);
    setError(null);
  }, []);

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="px-3 py-2 border-b border-border flex items-center gap-2 flex-wrap">
        {/* Browser buttons */}
        {BROWSERS.map((b) => (
          <Button
            key={b.kind}
            variant={selectedBrowser === b.kind ? "default" : "secondary"}
            size="sm"
            onClick={() => handleBrowserSwitch(b.kind)}
          >
            {b.label}
          </Button>
        ))}

        <span className="mx-1 text-border">|</span>

        {/* Profile select */}
        <select
          className="h-8 rounded-md border border-border bg-bg-elev-1 px-2 text-sm text-fg-primary"
          value={selectedProfile ?? ""}
          onChange={(e) => handleProfileChange(e.target.value)}
        >
          <option value="" disabled>
            {t("browser-forensics.select-profile")}
          </option>
          {profiles
            .filter((p) => p.browser === selectedBrowser)
            .map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
        </select>

        {/* Scan button — 按需触发，避免隐私争议与 EDR 检测 */}
        <Button
          variant="default"
          size="sm"
          disabled={loading || !selectedProfile}
          onClick={runScan}
          className="gap-1.5"
        >
          <ScanLine className="h-3.5 w-3.5" />
          {loading ? t("common.scanning", { defaultValue: "扫描中..." }) : t("browser-forensics.scan", { defaultValue: "扫描" })}
        </Button>

        <span className="mx-1 text-border">|</span>

        {/* Tab buttons */}
        {TAB_KEYS.map((tab) => (
          <Button
            key={tab.key}
            variant={activeTab === tab.key ? "default" : "secondary"}
            size="sm"
            onClick={() => setActiveTab(tab.key)}
          >
            {t(tab.i18nKey)}
          </Button>
        ))}

        <div className="flex-1" />

        {/* Search */}
        <Input
          className="w-48 h-8 text-sm"
          placeholder={t("browser-forensics.search")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      {/* Error */}
      {error && (
        <div className="px-3 py-1.5 bg-danger/10 text-danger text-xs flex items-center gap-2">
          <span className="flex-1">{error}</span>
          <button className="text-danger/60 hover:text-danger" onClick={() => setError(null)}>✕</button>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="px-3 py-1.5 bg-accent/10 text-accent text-xs flex items-center gap-2">
          <ScanLine className="h-3 w-3 animate-pulse" />
          {t("common.scanning", { defaultValue: "正在扫描浏览器数据..." })}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 min-h-0">
        {activeTab === "extensions" ? (
          <Group orientation="horizontal">
            <Panel defaultSize={60} minSize={30}>
              <ExtensionTable
                data={extensions}
                onRowSelect={handleExtensionSelect}
                selectedRowId={selectedExtensionId}
                search={search}
              />
            </Panel>
            {selectedExtensionId != null && (
              <>
                <Separator className="w-px bg-border hover:bg-accent transition-colors" />
                <Panel defaultSize={40} minSize={20}>
                  <ExtensionDetail item={selectedExtension} onClose={() => setSelectedExtensionId(null)} />
                </Panel>
              </>
            )}
          </Group>
        ) : activeTab === "history" ? (
          <HistoryTable data={history} search={search} />
        ) : activeTab === "downloads" ? (
          <DownloadTable data={downloads} search={search} />
        ) : activeTab === "context" ? (
          <ContextAttributionPanel />
        ) : (
          <TabTable data={tabs} search={search} />
        )}
      </div>
    </div>
  );
}
