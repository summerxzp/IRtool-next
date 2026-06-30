import { useEffect, useMemo, useRef, useCallback, useState, type ReactNode } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { ScanLine, Puzzle } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useBrowserForensicsStore, shouldUpgradeToConfirmed, urlToHostname, hostnameMatchesDomain } from "../store";
import * as api from "../api";
import { ExtensionTable } from "../components/ExtensionTable";
import { ExtensionDetail } from "../components/ExtensionDetail";
import { HistoryTable } from "../components/HistoryTable";
import { AttributionHistoryView } from "../components/AttributionHistoryView";
import { DownloadTable } from "../components/DownloadTable";
import { ContextAttributionPanel } from "../components/ContextAttributionPanel";
import { ExtensionConnectionBadge } from "../components/ExtensionConnectionBadge";
import { CdpCaptureControl } from "../components/CdpCaptureControl";
import { InstallHelperExtensionDialog } from "../components/InstallHelperExtensionDialog";
import type { BrowserKind, ExtensionInfo, BrowserMaliciousConnectionPayload, ExtensionAttributionPayload } from "../types";

const BROWSERS: { kind: BrowserKind; label: string }[] = [
  { kind: "chrome", label: "Chrome" },
  { kind: "edge", label: "Edge" },
];

// ── 可折叠行（二级菜单） ───────────────────────────────────────
// 默认收起，点击展开。展开后内容区固定高度，内部滚动。
// 可选内置搜索框（searchPlaceholder 提供时显示）。
function CollapsibleRow({
  title,
  count,
  defaultOpen = false,
  searchPlaceholder,
  search,
  onSearchChange,
  extraControls,
  children,
}: {
  title: string;
  count?: number;
  defaultOpen?: boolean;
  searchPlaceholder?: string;
  search?: string;
  onSearchChange?: (v: string) => void;
  extraControls?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-b border-border shrink-0">
      <button
        className="w-full flex items-center gap-2 px-3 py-2 text-sm hover:bg-bg-elev-1 transition-colors"
        onClick={() => setOpen(!open)}
      >
        <span className={`transform transition-transform text-xs text-muted-foreground ${open ? "rotate-90" : ""}`}>▶</span>
        <span className="font-medium text-fg-primary select-none">{title}</span>
        {count != null && count > 0 && (
          <span className="text-xs text-muted-foreground select-none">({count})</span>
        )}
      </button>
      {open && (
        <div className="border-t border-border">
          {(searchPlaceholder !== undefined || extraControls) && (
            <div className="px-3 py-1.5 flex items-center gap-2 border-b border-border flex-wrap">
              {searchPlaceholder !== undefined && (
                <Input
                  className="h-7 text-xs w-56"
                  placeholder={searchPlaceholder}
                  value={search ?? ""}
                  onChange={(e) => onSearchChange?.(e.target.value)}
                />
              )}
              {extraControls}
            </div>
          )}
          {children}
        </div>
      )}
    </div>
  );
}

export function BrowserForensicsPage() {
  const { t } = useTranslation();

  const selectedBrowser = useBrowserForensicsStore((s) => s.selectedBrowser);
  const setSelectedBrowser = useBrowserForensicsStore((s) => s.setSelectedBrowser);
  const selectedProfile = useBrowserForensicsStore((s) => s.selectedProfile);
  const setSelectedProfile = useBrowserForensicsStore((s) => s.setSelectedProfile);
  const profiles = useBrowserForensicsStore((s) => s.profiles);
  const setProfiles = useBrowserForensicsStore((s) => s.setProfiles);
  const setActiveTab = useBrowserForensicsStore((s) => s.setActiveTab);
  const extensions = useBrowserForensicsStore((s) => s.extensions);
  const setExtensions = useBrowserForensicsStore((s) => s.setExtensions);
  const downloads = useBrowserForensicsStore((s) => s.downloads);
  const setDownloads = useBrowserForensicsStore((s) => s.setDownloads);
  const history = useBrowserForensicsStore((s) => s.history);
  const setHistory = useBrowserForensicsStore((s) => s.setHistory);
  const setHistorySince = useBrowserForensicsStore((s) => s.setHistorySince);
  const historySince = useBrowserForensicsStore((s) => s.historySince);
  const loading = useBrowserForensicsStore((s) => s.loading);
  const setLoading = useBrowserForensicsStore((s) => s.setLoading);
  const error = useBrowserForensicsStore((s) => s.error);
  const setError = useBrowserForensicsStore((s) => s.setError);
  const selectedExtensionId = useBrowserForensicsStore((s) => s.selectedExtensionId);
  const setSelectedExtensionId = useBrowserForensicsStore((s) => s.setSelectedExtensionId);
  const setDomainAttribution = useBrowserForensicsStore((s) => s.setDomainAttribution);
  const setContextResult = useBrowserForensicsStore((s) => s.setContextResult);
  const setContextLoading = useBrowserForensicsStore((s) => s.setContextLoading);

  // History Attribution state
  const historyAttribution = useBrowserForensicsStore((s) => s.historyAttribution);
  const setHistoryAttribution = useBrowserForensicsStore((s) => s.setHistoryAttribution);
  const saveScanCache = useBrowserForensicsStore((s) => s.saveScanCache);
  const loadScanCache = useBrowserForensicsStore((s) => s.loadScanCache);
  const [anchorTime, setAnchorTime] = useState(() => new Date().toISOString());
  const [attributionLoading, setAttributionLoading] = useState(false);
  const [installDialogOpen, setInstallDialogOpen] = useState(false);

  // 二级菜单 Section 内部搜索状态（独立于 store，避免互相干扰）
  const [extSearch, setExtSearch] = useState("");
  const [historySearch, setHistorySearch] = useState("");
  const [downloadSearch, setDownloadSearch] = useState("");

  const runAttribution = useCallback(async () => {
    if (!selectedProfile) return;
    setAttributionLoading(true);
    try {
      const result = await api.getHistory(selectedBrowser, selectedProfile, anchorTime);
      setHistoryAttribution(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setAttributionLoading(false);
    }
  }, [selectedBrowser, selectedProfile, anchorTime, setHistoryAttribution, setError]);

  // Load profiles on mount
  useEffect(() => {
    api.listProfiles().then(setProfiles);
  }, [setProfiles]);

  // 启动时从磁盘 config.json 同步已下发的 filterDomains 到 UI（由 ContextAttributionPanel 负责）

  // Auto-select first profile on mount
  useEffect(() => {
    if (profiles.length > 0 && !selectedProfile) {
      setSelectedProfile(profiles[0].name);
    }
  }, [profiles, selectedProfile, setSelectedProfile]);

  // ── 监听后端推送的浏览器恶意连接事件，自动触发上下文归因 ──
  useEffect(() => {
    const unlistenPromise = listen<BrowserMaliciousConnectionPayload>(
      "evt_browser_malicious_connection",
      (event) => {
        const { domain, ip, process_name, pid, cmdline } = event.payload;
        console.info("[browser-forensics] malicious connection detected:", event.payload);

        // 主视图始终是上下文归因，无需切换 tab；保留调用作为意图标记
        setActiveTab("context");
        setContextLoading(true);

        api.attributeBrowserContext(domain, ip || null, process_name, pid, cmdline ?? undefined)
          .then((result) => {
            if (result) {
              const existingEvents = useBrowserForensicsStore.getState().extensionAttributions;
              const merged = result.extension_attribution && shouldUpgradeToConfirmed(existingEvents, result.domain)
                ? {
                    ...result,
                    extension_attribution: {
                      ...result.extension_attribution,
                      confidence: "confirmed" as const,
                    },
                  }
                : result;
              setContextResult(merged);
            }
            setContextLoading(false);
          })
          .catch(() => {
            setContextResult(null);
            setContextLoading(false);
          });

        const browser: BrowserKind = process_name?.includes("edge") ? "edge" : "chrome";
        api.attributeByDomain(domain, browser).then((result) => {
          setDomainAttribution(result);
        }).catch(() => {
          // ignore supplemental error
        });
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── 监听 Helper Extension 上报的扩展归因事件 ──────────
  const addExtensionAttribution = useBrowserForensicsStore((s) => s.addExtensionAttribution);
  const upgradeContextExtensionConfidence = useBrowserForensicsStore((s) => s.upgradeContextExtensionConfidence);
  const contextResultDomain = useBrowserForensicsStore((s) => s.contextResult?.domain ?? null);
  const domainRef = useRef<string | null>(null);
  useEffect(() => {
    domainRef.current = contextResultDomain;
  }, [contextResultDomain]);
  useEffect(() => {
    const unlistenPromise = listen<ExtensionAttributionPayload>(
      "evt_extension_attribution",
      (event) => {
        addExtensionAttribution(event.payload);
        const currentDomain = domainRef.current;
        if (event.payload.level === "confirmed" && currentDomain) {
          const hostname = urlToHostname(event.payload.url);
          if (hostname && hostnameMatchesDomain(hostname, currentDomain)) {
            upgradeContextExtensionConfidence(currentDomain);
          }
        }
      },
    );
    return () => {
      unlistenPromise.then((fn) => fn());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── 扫描：按需触发 ──────────────────────
  const reqIdRef = useRef(0);
  const runScan = useCallback(() => {
    if (!selectedProfile) return;
    const reqId = ++reqIdRef.current;

    setExtensions([]);
    setDownloads([]);
    setHistory([]);
    setSelectedExtensionId(null);
    setHistoryAttribution(null);
    setLoading(true);
    setError(null);

    const isCurrent = () => reqId === reqIdRef.current;

    const sinceWebKit = (() => {
      const now = Date.now();
      let sinceMs: number | undefined;
      switch (historySince) {
        case "1h": sinceMs = now - 3600000; break;
        case "24h": sinceMs = now - 86400000; break;
        case "7d": sinceMs = now - 604800000; break;
        default: return undefined;
      }
      return Math.floor((sinceMs + 11644473600000) * 1000);
    })();

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
      api.scanHistory(selectedBrowser, selectedProfile, sinceWebKit).then((h) => {
        if (isCurrent()) setHistory(h);
      }).catch((e) => { if (isCurrent()) setError(String(e)); }),
    );

    Promise.all(fetches).finally(() => {
      if (isCurrent()) {
        setLoading(false);
        // 保存扫描结果到缓存，切换浏览器/profile 后可恢复
        const state = useBrowserForensicsStore.getState();
        saveScanCache(selectedBrowser, selectedProfile, {
          extensions: state.extensions,
          downloads: state.downloads,
          history: state.history,
          historySince: state.historySince,
        });
      }
    });
  }, [selectedBrowser, selectedProfile, historySince]);

  const selectedExtension = useMemo(
    () => extensions.find((e) => e.id === selectedExtensionId) ?? null,
    [extensions, selectedExtensionId],
  );

  const handleExtensionSelect = (row: ExtensionInfo | null) => {
    setSelectedExtensionId(row?.id ?? null);
  };

  const handleBrowserSwitch = useCallback((kind: BrowserKind) => {
    if (kind === selectedBrowser) return;
    setSelectedBrowser(kind);
    const firstProfile = profiles.find((p) => p.browser === kind);
    const profileName = firstProfile?.name ?? null;
    setSelectedProfile(profileName);
    setHistoryAttribution(null);
    setError(null);
    // 从缓存恢复数据（切回来时数据还在），无缓存则清空
    if (profileName) {
      loadScanCache(kind, profileName);
    } else {
      setExtensions([]);
      setDownloads([]);
      setHistory([]);
      setSelectedExtensionId(null);
    }
  }, [selectedBrowser, profiles]);

  const handleProfileChange = useCallback((value: string) => {
    setSelectedProfile(value);
    setHistoryAttribution(null);
    setError(null);
    // 从缓存恢复数据
    loadScanCache(selectedBrowser, value);
  }, [selectedBrowser]);

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar — 精简：左侧浏览器/profile/扫描，右侧基础设施工具 */}
      <div className="px-3 py-2 border-b border-border flex items-center gap-2">
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

        <div className="flex-1" />

        {/* 基础设施工具：CDP / 连接徽标 / 安装按钮（常驻但精简） */}
        <CdpCaptureControl />
        <ExtensionConnectionBadge />
        <Button
          variant="secondary"
          size="sm"
          className="gap-1.5"
          onClick={() => setInstallDialogOpen(true)}
        >
          <Puzzle className="h-3.5 w-3.5" />
          {t("browser-forensics.install-helper.button")}
        </Button>
      </div>

      {/* 安装 Helper Extension 引导对话框 */}
      <InstallHelperExtensionDialog open={installDialogOpen} onOpenChange={setInstallDialogOpen} />

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

      {/* 主内容区：上下文归因（主视图） + 下方可折叠二级菜单 */}
      <div className="flex-1 min-h-0 flex flex-col">
        {/* 主视图：上下文归因（常驻，占满剩余空间，保证最小高度） */}
        <div className="flex-1 min-h-[200px]">
          <ContextAttributionPanel />
        </div>

        {/* 二级菜单：可折叠 Section（默认收起） */}
        <div className="border-t border-border shrink-0 max-h-[60vh] overflow-y-auto">
          {/* 扩展库 */}
          <CollapsibleRow
            title={t("browser-forensics.extensions", { defaultValue: "扩展库" })}
            count={extensions.length}
            searchPlaceholder={t("browser-forensics.search")}
            search={extSearch}
            onSearchChange={setExtSearch}
          >
            <div className="h-[45vh]">
              <Group orientation="horizontal">
                <Panel defaultSize={60} minSize={30}>
                  <ExtensionTable
                    data={extensions}
                    onRowSelect={handleExtensionSelect}
                    selectedRowId={selectedExtensionId}
                    search={extSearch}
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
            </div>
          </CollapsibleRow>

          {/* 浏览历史（含归因分析控件） */}
          <CollapsibleRow
            title={t("browser-forensics.history", { defaultValue: "浏览历史" })}
            count={history.length}
            searchPlaceholder={t("browser-forensics.search")}
            search={historySearch}
            onSearchChange={setHistorySearch}
            extraControls={
              <>
                <select
                  className="h-7 rounded-md border border-border bg-bg-elev-1 px-2 text-xs text-fg-primary"
                  value={historySince}
                  onChange={(e) => setHistorySince(e.target.value)}
                >
                  <option value="all">{t("browser-forensics.history-range-all", { defaultValue: "全部" })}</option>
                  <option value="1h">{t("browser-forensics.history-range-1h", { defaultValue: "最近 1 小时" })}</option>
                  <option value="24h">{t("browser-forensics.history-range-24h", { defaultValue: "最近 24 小时" })}</option>
                  <option value="7d">{t("browser-forensics.history-range-7d", { defaultValue: "最近 7 天" })}</option>
                </select>
                <Input
                  className="w-44 h-7 text-xs font-mono"
                  value={anchorTime}
                  onChange={(e) => setAnchorTime(e.target.value)}
                  placeholder={t("browser-forensics.attribution-anchor-time", { defaultValue: "锚点时间 (ISO 8601)" })}
                />
                <Button
                  variant="secondary"
                  size="sm"
                  className="h-7 text-xs"
                  onClick={() => setAnchorTime(new Date().toISOString())}
                >
                  {t("browser-forensics.attribution-now", { defaultValue: "现在" })}
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  className="h-7 text-xs"
                  disabled={attributionLoading || !selectedProfile}
                  onClick={runAttribution}
                >
                  {attributionLoading
                    ? t("common.scanning", { defaultValue: "分析中..." })
                    : t("browser-forensics.attribution-analyze", { defaultValue: "归因分析" })}
                </Button>
              </>
            }
          >
            <div className="h-[40vh] flex flex-col">
              <div className="flex-1 min-h-0">
                <HistoryTable data={history} search={historySearch} />
              </div>
              {(historyAttribution || attributionLoading) && (
                <div className="border-t border-border max-h-[45%] min-h-0 overflow-y-auto">
                  <AttributionHistoryView attribution={historyAttribution} loading={attributionLoading} />
                </div>
              )}
            </div>
          </CollapsibleRow>

          {/* 下载记录 */}
          <CollapsibleRow
            title={t("browser-forensics.downloads", { defaultValue: "下载记录" })}
            count={downloads.length}
            searchPlaceholder={t("browser-forensics.search")}
            search={downloadSearch}
            onSearchChange={setDownloadSearch}
          >
            <div className="h-[45vh]">
              <DownloadTable data={downloads} search={downloadSearch} />
            </div>
          </CollapsibleRow>
        </div>
      </div>
    </div>
  );
}
