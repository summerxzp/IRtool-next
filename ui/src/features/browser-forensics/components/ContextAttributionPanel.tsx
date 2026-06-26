import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useBrowserForensicsStore } from "../store";
import * as api from "../api";
import type {
  DomainAttribution, BrowserKind, MatchedExtension, CurrentTab,
  HistoryEntry, DownloadInfo, BrowserContext, RecentActivity,
  NavChainNode,
} from "../types";
import { formatTimestamp } from "../utils";

// ── 颜色常量 ──────────────────────────────────────────────────────

const TIER_COLORS: Record<string, string> = {
  immediate: "#ef4444",
  nearby: "#eab308",
  recent: "#6b7280",
};

// ── 工具函数 ───────────────────────────────────────────────────────

function formatEventTimestamp(ts: number): string {
  try {
    const d = new Date(ts);
    if (isNaN(d.getTime())) return String(ts);
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    const h = String(d.getHours()).padStart(2, "0");
    const min = String(d.getMinutes()).padStart(2, "0");
    const s = String(d.getSeconds()).padStart(2, "0");
    return `${y}/${m}/${day} ${h}:${min}:${s}`;
  } catch {
    return String(ts);
  }
}

// ── 通用子组件 ────────────────────────────────────────────────────

function RiskBadge({ flags }: { flags: string[] }) {
  if (flags.length === 0) return null;
  const { t } = useTranslation();
  return (
    <span className="inline-flex gap-1 flex-wrap">
      {flags.map((f) => (
        <span
          key={f}
          className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-warning/15 text-warning"
        >
          {t(`browser-forensics.risk.${f.replace(/_/g, "-")}`, { defaultValue: f })}
        </span>
      ))}
    </span>
  );
}

function MatchingExtensionsView({ extensions: exts }: { extensions: MatchedExtension[] }) {
  const { t } = useTranslation();
  if (exts.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-matching-extensions", { defaultValue: "无匹配扩展" })}</div>;
  }
  return (
    <div className="space-y-1">
      {exts.map((ext) => (
        <div key={ext.id} className="flex items-start gap-2 p-1.5 rounded bg-bg-elev-1 text-xs">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="font-medium text-fg-primary truncate">{ext.name}</span>
              <span className="text-muted-foreground shrink-0">v{ext.version}</span>
            </div>
            <div className="text-[10px] text-muted-foreground font-mono truncate">{ext.id}</div>
            {ext.matched_patterns.length > 0 && (
              <div className="mt-0.5 text-[10px] text-code">
                Match: {ext.matched_patterns.join(", ")}
              </div>
            )}
            {ext.has_sensitive_permissions && (
              <div className="mt-0.5 text-[10px] text-danger">
                ⚠ Sensitive: webRequest + &lt;all_urls&gt;
              </div>
            )}
            <RiskBadge flags={ext.risk_flags} />
          </div>
        </div>
      ))}
    </div>
  );
}

function RelatedHistoryView({ entries }: { entries: HistoryEntry[] }) {
  if (entries.length === 0) {
    return <div className="text-xs text-muted-foreground italic">无相关浏览记录</div>;
  }
  return (
    <div className="space-y-0.5 max-h-48 overflow-y-auto">
      {entries.map((entry, i) => (
        <div key={i} className="flex items-start gap-2 text-xs">
          <span className="text-muted-foreground shrink-0 w-4 text-right">{i + 1}.</span>
          <div className="min-w-0 flex-1">
            <div className="text-fg-primary truncate" title={entry.url}>{entry.title || entry.url}</div>
            <div className="text-[10px] text-muted-foreground truncate">{entry.url}</div>
            <div className="flex gap-2 text-[10px] text-muted-foreground">
              <span>{formatTimestamp(entry.visit_time)}</span>
              <span>×{entry.visit_count}</span>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function RelatedDownloadsView({ downloads }: { downloads: DownloadInfo[] }) {
  if (downloads.length === 0) {
    return <div className="text-xs text-muted-foreground italic">无相关下载记录</div>;
  }
  return (
    <div className="space-y-0.5 max-h-32 overflow-y-auto">
      {downloads.map((d, i) => (
        <div key={i} className="flex items-start gap-2 text-xs">
          <span className="text-muted-foreground shrink-0 w-4 text-right">{i + 1}.</span>
          <div className="min-w-0 flex-1">
            <div className="text-fg-primary truncate">{d.filename}</div>
            <div className="text-[10px] text-muted-foreground truncate">{d.download_url}</div>
            <div className="flex gap-2 text-[10px] text-muted-foreground">
              {d.start_time && <span>{formatTimestamp(d.start_time)}</span>}
              {d.total_bytes != null && <span>{(d.total_bytes / 1024).toFixed(1)}KB</span>}
              {d.danger_type !== "NOT_DANGEROUS" && (
                <span className="text-danger">{d.danger_type}</span>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function RelatedTabsView({ tabs }: { tabs: CurrentTab[] }) {
  if (tabs.length === 0) {
    return <div className="text-xs text-muted-foreground italic">无相关标签页</div>;
  }
  return (
    <div className="space-y-0.5">
      {tabs.map((tab, i) => (
        <div key={i} className="flex items-center gap-2 text-xs">
          {tab.active && <span className="text-success shrink-0">●</span>}
          <div className="min-w-0 flex-1 truncate" title={tab.url}>{tab.title || tab.url}</div>
        </div>
      ))}
    </div>
  );
}

function DomainAttributionCard({ result }: { result: DomainAttribution }) {
  const { t } = useTranslation();
  const hasAnyData = result.matching_extensions.length > 0
    || result.related_history.length > 0
    || result.related_downloads.length > 0
    || result.related_tabs.length > 0;

  return (
    <div className="rounded-lg border border-border bg-bg-elev-1 p-3">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {result.browser} / {result.profile}
        </span>
        {!hasAnyData && (
          <span className="text-[10px] text-muted-foreground italic">未发现相关痕迹</span>
        )}
      </div>

      {hasAnyData && (
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("browser-forensics.context.matching-extensions", { defaultValue: "可疑扩展" })}
            </h4>
            <MatchingExtensionsView extensions={result.matching_extensions} />
          </div>
          <div className="space-y-1.5">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("browser-forensics.context.related-tabs", { defaultValue: "相关标签页" })}
            </h4>
            <RelatedTabsView tabs={result.related_tabs} />
          </div>
          <div className="space-y-1.5">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("browser-forensics.context.related-history", { defaultValue: "相关浏览记录" })}
            </h4>
            <RelatedHistoryView entries={result.related_history} />
          </div>
          <div className="space-y-1.5">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("browser-forensics.context.related-downloads", { defaultValue: "相关下载记录" })}
            </h4>
            <RelatedDownloadsView downloads={result.related_downloads} />
          </div>
        </div>
      )}
    </div>
  );
}

// ── Collapsible Section wrapper ───────────────────────────────────

function CollapsibleSection({ title, defaultOpen = true, children }: { title: string; defaultOpen?: boolean; children: React.ReactNode }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="rounded-lg border border-border bg-bg-base mb-2">
      <button
        className="w-full flex items-center justify-between px-3 py-2 text-sm font-semibold text-fg-primary hover:bg-bg-elev-1 transition-colors rounded-lg"
        onClick={() => setOpen(!open)}
      >
        <span>{title}</span>
        <span className={`transform transition-transform ${open ? "rotate-180" : ""}`}>▼</span>
      </button>
      {open && <div className="px-3 pb-3 space-y-3">{children}</div>}
    </div>
  );
}

// ── Section A 子组件 ──────────────────────────────────────────────

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-1.5">
      <span className="text-muted-foreground shrink-0">{label}:</span>
      <span className="text-fg-primary font-mono truncate">{value}</span>
    </div>
  );
}

function ConnectionInfoCard({ connection }: { connection: BrowserContext["malicious_connection"] }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-lg border border-border bg-bg-elev-1 p-3 space-y-1.5">
      <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        {t("browser-forensics.context.connection-info", { defaultValue: "连接信息" })}
      </h4>
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
        <InfoRow label="Domain" value={connection.domain} />
        <InfoRow label="IP" value={connection.ip ?? "-"} />
        <InfoRow label={t("browser-forensics.context.process-name", { defaultValue: "进程名" })} value={connection.process} />
        <InfoRow label={t("browser-forensics.context.pid", { defaultValue: "PID" })} value={String(connection.pid)} />
        <InfoRow label={t("browser-forensics.col.browser-profile", { defaultValue: "浏览器 / 配置文件" })} value={`${connection.browser} / ${connection.profile}`} />
        <InfoRow label={t("browser-forensics.context.connection", { defaultValue: "连接" })} value={formatTimestamp(connection.timestamp)} />
      </div>
    </div>
  );
}

function TierBadge({ tier }: { tier: string }) {
  const color = TIER_COLORS[tier] ?? "#6b7280";
  const { t } = useTranslation();
  const labels: Record<string, string> = {
    immediate: t("browser-forensics.tier-immediate", { defaultValue: "紧邻" }),
    nearby: t("browser-forensics.tier-nearby", { defaultValue: "附近" }),
    recent: t("browser-forensics.tier-recent", { defaultValue: "近期" }),
  };
  return (
    <span
      className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider"
      style={{
        backgroundColor: `${color}20`,
        color,
        border: `1px solid ${color}40`,
      }}
    >
      {labels[tier] ?? tier}
    </span>
  );
}

function ActivityCard({ activity }: { activity: RecentActivity }) {
  return (
    <div className="flex items-start gap-2 p-2 rounded bg-bg-elev-1 border border-border text-xs">
      <TierBadge tier={activity.tier} />
      <div className="min-w-0 flex-1">
        <div className="text-fg-primary truncate font-medium" title={activity.title}>
          {activity.title || activity.url}
        </div>
        <div className="text-[10px] text-muted-foreground truncate font-mono" title={activity.url}>
          {activity.url}
        </div>
        <div className="flex gap-2 text-[10px] text-muted-foreground mt-0.5">
          <span>{formatTimestamp(activity.visit_time)}</span>
        </div>
      </div>
    </div>
  );
}

function TierGroup({ tier, activities }: { tier: string; activities: RecentActivity[] }) {
  const color = TIER_COLORS[tier] ?? "#6b7280";
  const { t } = useTranslation();
  const labels: Record<string, string> = {
    immediate: t("browser-forensics.tier-immediate", { defaultValue: "紧邻" }),
    nearby: t("browser-forensics.tier-nearby", { defaultValue: "附近" }),
    recent: t("browser-forensics.tier-recent", { defaultValue: "近期" }),
  };
  if (activities.length === 0) return null;

  return (
    <div
      className="rounded-lg border p-3 space-y-2"
      style={{
        borderColor: `${color}30`,
        backgroundColor: `${color}08`,
      }}
    >
      <div className="flex items-center gap-2 mb-1">
        <span className="w-2.5 h-2.5 rounded-full inline-block" style={{ backgroundColor: color }} />
        <span className="text-xs font-semibold uppercase tracking-wider" style={{ color }}>
          {labels[tier] ?? tier}
        </span>
      </div>
      <div className="space-y-1.5">
        {activities.map((activity, idx) => (
          <ActivityCard key={`${activity.url}-${activity.visit_time}-${idx}`} activity={activity} />
        ))}
      </div>
    </div>
  );
}

function NavChainTimeline({ nodes }: { nodes: NavChainNode[] }) {
  const { t } = useTranslation();
  if (nodes.length === 0) {
    return (
      <div className="text-xs text-muted-foreground italic py-4 text-center">
        {t("browser-forensics.context.no-navigation-chain", { defaultValue: "未找到导航链" })}
      </div>
    );
  }
  return (
    <div className="relative pl-6 space-y-0">
      {nodes.map((node, idx) => (
        <div key={idx} className="relative pb-4 last:pb-0">
          {idx < nodes.length - 1 && (
            <div className="absolute left-[7px] top-3 bottom-0 w-0.5 bg-border" />
          )}
          <div className="absolute left-0 top-1.5 w-[15px] flex items-center justify-center">
            <div className="w-2.5 h-2.5 rounded-full border-2 border-accent bg-bg-base" />
          </div>
          <div className="ml-3 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {node.transition && (
                <span
                  className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono font-medium"
                  style={{
                    backgroundColor: `#6b728020`,
                    color: "#6b7280",
                    border: "1px solid #6b728040",
                  }}
                >
                  {node.transition}
                </span>
              )}
            </div>
            <div className="text-xs text-fg-primary font-medium truncate" title={node.title ?? undefined}>
              {node.title || "(no title)"}
            </div>
            <div className="text-[10px] text-muted-foreground truncate font-mono" title={node.url}>
              {node.url}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function BrowserCurrentTabsView({ tabs }: { tabs: CurrentTab[] }) {
  const { t } = useTranslation();
  if (tabs.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-recovered-tabs", { defaultValue: "无恢复标签页" })}</div>;
  }
  return (
    <div className="space-y-0.5 max-h-32 overflow-y-auto">
      {tabs.map((tab, i) => (
        <div key={i} className="flex items-center gap-2 text-xs p-1 rounded hover:bg-bg-elev-1">
          {tab.active && <span className="text-success shrink-0 text-[10px]">●</span>}
          <div className="min-w-0 flex-1 truncate" title={tab.url}>{tab.title || tab.url}</div>
          <span className="text-[10px] text-muted-foreground">{tab.evidence_type}</span>
        </div>
      ))}
    </div>
  );
}

function BrowserRelatedDownloadsView({ downloads }: { downloads: DownloadInfo[] }) {
  if (downloads.length === 0) {
    return <div className="text-xs text-muted-foreground italic">无相关下载记录</div>;
  }
  return (
    <div className="space-y-0.5 max-h-32 overflow-y-auto">
      {downloads.map((d, i) => (
        <div key={i} className="flex items-start gap-2 text-xs p-1 rounded hover:bg-bg-elev-1">
          <div className="min-w-0 flex-1">
            <div className="text-fg-primary truncate">{d.filename}</div>
            <div className="text-[10px] text-muted-foreground truncate">{d.download_url}</div>
          </div>
        </div>
      ))}
    </div>
  );
}

function BrowserMatchingExtensionsView({ extensions: exts }: { extensions: MatchedExtension[] }) {
  const { t } = useTranslation();
  if (exts.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-matching-extensions", { defaultValue: "无匹配扩展" })}</div>;
  }
  return (
    <div className="space-y-1">
      {exts.map((ext) => (
        <div key={ext.id} className="flex items-start gap-2 p-1.5 rounded bg-bg-elev-1 text-xs">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="font-medium text-fg-primary truncate">{ext.name}</span>
              <span className="text-muted-foreground shrink-0">v{ext.version}</span>
            </div>
            <div className="text-[10px] text-muted-foreground font-mono truncate">{ext.id}</div>
            {ext.matched_patterns.length > 0 && (
              <div className="mt-0.5 text-[10px] text-code">Match: {ext.matched_patterns.join(", ")}</div>
            )}
            {ext.has_sensitive_permissions && (
              <div className="mt-0.5 text-[10px] text-danger">⚠ Sensitive: webRequest + &lt;all_urls&gt;</div>
            )}
            <RiskBadge flags={ext.risk_flags} />
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Section A: Connection Attribution Card ───────────────────────

function ConnectionAttributionCard({ result }: { result: BrowserContext }) {
  const { t } = useTranslation();
  const { malicious_connection, context } = result;

  // 按 tier 分组
  const grouped: Record<string, RecentActivity[]> = {};
  for (const activity of context.recent_browser_activity) {
    const tier = activity.tier;
    if (!grouped[tier]) grouped[tier] = [];
    grouped[tier].push(activity);
  }
  const tierOrder = ["immediate", "nearby", "recent"];

  return (
    <div className="px-3 pt-3">
      <CollapsibleSection
        title={t("browser-forensics.context.connection-attribution", { defaultValue: "恶意连接归因" })}
        defaultOpen
      >
        {/* 连接信息 */}
        <ConnectionInfoCard connection={malicious_connection} />

        {/* 分层活动 */}
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
            {t("browser-forensics.context.recent-activity", { defaultValue: "最近活动" })}
          </h4>
          <div className="space-y-2">
            {tierOrder.map((tier) => (
              <TierGroup key={tier} tier={tier} activities={grouped[tier] ?? []} />
            ))}
            {context.recent_browser_activity.length === 0 && (
              <div className="text-xs text-muted-foreground italic py-2 text-center">
                {t("browser-forensics.context.no-recent-activity", { defaultValue: "无最近活动" })}
              </div>
            )}
          </div>
        </div>

        {/* 导航链 */}
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
            {t("browser-forensics.context.navigation-chain", { defaultValue: "导航链" })}
          </h4>
          <NavChainTimeline nodes={context.navigation_chain} />
        </div>

        {/* 当前标签页 */}
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
            {t("browser-forensics.context.current-tabs", { defaultValue: "当前标签页" })}
          </h4>
          <BrowserCurrentTabsView tabs={context.current_tabs} />
        </div>

        {/* 相关下载记录 */}
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
            {t("browser-forensics.context.related-downloads", { defaultValue: "相关下载记录" })}
          </h4>
          <BrowserRelatedDownloadsView downloads={context.recent_downloads} />
        </div>

        {/* 匹配扩展 */}
        <div>
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
            {t("browser-forensics.context.matching-extensions", { defaultValue: "匹配扩展" })}
          </h4>
          <BrowserMatchingExtensionsView extensions={context.matching_extensions} />
        </div>
      </CollapsibleSection>
    </div>
  );
}

// ── Section B: Extension Events ──────────────────────────────────

function ExtensionEventsCard() {
  const { t } = useTranslation();
  const extensionAttributions = useBrowserForensicsStore((s) => s.extensionAttributions);
  const clearExtensionAttributions = useBrowserForensicsStore((s) => s.clearExtensionAttributions);

  const sorted = [...extensionAttributions].reverse().slice(0, 10);

  return (
    <div className="px-3 pb-3">
      <CollapsibleSection
        title={t("browser-forensics.context.extension-events", { defaultValue: "实时扩展归因事件" })}
        defaultOpen={false}
      >
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs text-muted-foreground">
            {t("browser-forensics.context.extension-events-count", { defaultValue: "共 {{count}} 条", count: extensionAttributions.length })}
          </span>
          {extensionAttributions.length > 0 && (
            <Button size="sm" variant="secondary" className="h-6 text-xs" onClick={clearExtensionAttributions}>
              {t("browser-forensics.context.clear", { defaultValue: "清除" })}
            </Button>
          )}
        </div>

        {extensionAttributions.length === 0 ? (
          <div className="text-xs text-muted-foreground italic py-4 text-center">
            {t("browser-forensics.context.extension-events-empty", { defaultValue: "暂无扩展归因事件" })}
          </div>
        ) : (
          <div className="space-y-1 max-h-80 overflow-y-auto">
            {sorted.map((evt, idx) => (
              <div
                key={`${evt.request_id}-${idx}`}
                className="flex items-start gap-2 p-2 rounded bg-bg-elev-1 border border-border text-xs"
              >
                <div className="min-w-0 flex-1 space-y-0.5">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-[10px] text-muted-foreground font-mono">
                      {formatEventTimestamp(evt.timestamp)}
                    </span>
                    <span className="px-1 py-0.5 rounded text-[10px] font-mono bg-muted/30 text-muted-foreground">
                      {evt.method}
                    </span>
                    <span
                      className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
                        evt.attribution_status === "matched"
                          ? "bg-success/15 text-success"
                          : "bg-muted/30 text-muted-foreground"
                      }`}
                    >
                      {evt.attribution_status === "matched"
                        ? t("browser-forensics.context.event-matched", { defaultValue: "已匹配" })
                        : t("browser-forensics.context.event-unmatched", { defaultValue: "未匹配" })}
                    </span>
                  </div>
                  <div className="text-fg-primary truncate font-medium" title={evt.url}>
                    {evt.url}
                  </div>
                  <div className="flex gap-3 text-[10px] text-muted-foreground">
                    {evt.extension_name && (
                      <span>
                        {t("browser-forensics.context.event-extension", { defaultValue: "扩展" })}: {evt.extension_name}
                      </span>
                    )}
                    {evt.initiator && (
                      <span>
                        {t("browser-forensics.context.event-initiator", { defaultValue: "发起者" })}: {evt.initiator}
                      </span>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </CollapsibleSection>
    </div>
  );
}

// ── 主面板 ──────────────────────────────────────────────────────

export function ContextAttributionPanel() {
  const { t } = useTranslation();

  const domainAttribution = useBrowserForensicsStore((s) => s.domainAttribution);
  const setDomainAttribution = useBrowserForensicsStore((s) => s.setDomainAttribution);
  const contextResult = useBrowserForensicsStore((s) => s.contextResult);
  const contextLoading = useBrowserForensicsStore((s) => s.contextLoading);
  const setContextLoading = useBrowserForensicsStore((s) => s.setContextLoading);

  // 手动输入的本地状态
  const [inputTarget, setInputTarget] = useState("");
  const [selectedBrowser, setSelectedBrowser] = useState<BrowserKind>("chrome");

  const handleAnalyze = async () => {
    if (!inputTarget.trim()) return;
    setContextLoading(true);
    const result = await api.attributeByDomain(inputTarget.trim(), selectedBrowser);
    setDomainAttribution(result);
    setContextLoading(false);
  };

  return (
    <div className="h-full flex flex-col">
      {/* Section A: 连接归因（BrowserContext） */}
      {contextResult && <ConnectionAttributionCard result={contextResult} />}

      {/* 输入区 */}
      <div className="px-3 py-2 border-b border-border flex items-center gap-2">
        <Input
          className="w-56 h-8 text-sm"
          placeholder={t("browser-forensics.context.domain-placeholder", { defaultValue: "输入域名或 IP" })}
          value={inputTarget}
          onChange={(e) => setInputTarget(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAnalyze()}
        />
        <Select value={selectedBrowser} onValueChange={(v) => setSelectedBrowser(v as BrowserKind)}>
          <SelectTrigger className="w-28 h-8 text-sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="chrome">Chrome</SelectItem>
            <SelectItem value="edge">Edge</SelectItem>
            <SelectItem value="brave">Brave</SelectItem>
            <SelectItem value="vivaldi">Vivaldi</SelectItem>
          </SelectContent>
        </Select>
        <Button size="sm" onClick={handleAnalyze} disabled={contextLoading || !inputTarget.trim()}>
          {contextLoading
            ? t("browser-forensics.context.analyzing", { defaultValue: "分析中..." })
            : t("browser-forensics.context.analyze", { defaultValue: "归因分析" })}
        </Button>
        {domainAttribution && (
          <Button size="sm" variant="secondary" onClick={() => setDomainAttribution(null)}>
            {t("browser-forensics.context.clear", { defaultValue: "清除" })}
          </Button>
        )}
      </div>

      {/* 结果区 */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {!domainAttribution && (
          <div className="p-6 text-sm text-muted-foreground text-center flex flex-col items-center gap-2 pt-12">
            <span className="text-2xl">🔍</span>
            <p>{t("browser-forensics.context.hint", { defaultValue: "输入目标域名/IP 进行归因分析，定位相关的浏览器扩展和浏览活动" })}</p>
          </div>
        )}

        {domainAttribution && domainAttribution.length === 0 && (
          <div className="p-6 text-sm text-muted-foreground text-center pt-12">
            未找到该浏览器的 Profile
          </div>
        )}

        {domainAttribution && domainAttribution.length > 0 && (
          <div className="p-3 space-y-3">
            {domainAttribution.map((result, i) => (
              <DomainAttributionCard key={i} result={result} />
            ))}
          </div>
        )}

        {/* Section B: 实时扩展归因事件 */}
        <div className="border-t border-border mt-2">
          <ExtensionEventsCard />
        </div>
      </div>
    </div>
  );
}
