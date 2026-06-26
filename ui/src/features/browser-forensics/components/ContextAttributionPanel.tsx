import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useBrowserForensicsStore } from "../store";
import * as api from "../api";
import type {
  DomainAttribution, BrowserKind, MatchedExtension, CurrentTab,
  HistoryEntry, DownloadInfo,
  EvidenceObject, AttributionLevel, EvidenceScore,
} from "../types";
import { formatTimestamp } from "../utils";

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

// ── Section A: EvidenceObjectView (P0.7b) ────────────────────────

function ConfidenceBadge({ level }: { level: AttributionLevel }) {
  const { t } = useTranslation();
  const styles: Record<AttributionLevel, string> = {
    confirmed: "bg-success/15 text-success border-success/40",
    probable: "bg-warning/15 text-warning border-warning/40",
    possible: "bg-muted/30 text-muted-foreground border-border",
  };
  const labels: Record<AttributionLevel, string> = {
    confirmed: t("browser-forensics.context.confidence.confirmed", { defaultValue: "已确认" }),
    probable: t("browser-forensics.context.confidence.probable", { defaultValue: "较可能" }),
    possible: t("browser-forensics.context.confidence.possible", { defaultValue: "可能" }),
  };
  return (
    <span
      className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider border ${styles[level]}`}
    >
      {labels[level]}
    </span>
  );
}

function ScoreBreakdown({ score }: { score: EvidenceScore }) {
  const { t } = useTranslation();
  const items: Array<{ label: string; value: number; bold?: boolean }> = [
    { label: t("browser-forensics.context.score.time", { defaultValue: "时间分" }), value: score.time_score },
    { label: t("browser-forensics.context.score.domain", { defaultValue: "域名分" }), value: score.domain_score },
    { label: t("browser-forensics.context.score.chain", { defaultValue: "链路分" }), value: score.chain_score },
    { label: t("browser-forensics.context.score.total", { defaultValue: "总分" }), value: score.total, bold: true },
  ];
  return (
    <div className="grid grid-cols-4 gap-2 text-xs">
      {items.map((it) => (
        <div key={it.label}>
          <div className="text-[10px] text-muted-foreground select-none">{it.label}</div>
          <div className={`font-mono text-fg-primary ${it.bold ? "font-semibold" : ""}`}>{it.value}</div>
        </div>
      ))}
    </div>
  );
}

function EvidenceObjectView({ result }: { result: EvidenceObject }) {
  const { t } = useTranslation();
  const {
    malicious_connection: conn,
    history_correlation: hc,
    navigation_chain: navChain,
    downloads,
    extension_attribution: extAttr,
    overall_confidence: overallConf,
    overall_score: overallScore,
  } = result;

  const tierColors: Record<string, string> = {
    immediate: "#ef4444",
    nearby: "#eab308",
    recent: "#6b7280",
  };
  const tierLabels: Record<string, string> = {
    immediate: t("browser-forensics.tier-immediate", { defaultValue: "紧邻" }),
    nearby: t("browser-forensics.tier-nearby", { defaultValue: "附近" }),
    recent: t("browser-forensics.tier-recent", { defaultValue: "近期" }),
  };

  return (
    <div className="px-3 pt-3">
      {/* 顶部：连接信息 + 综合置信度/评分 */}
      <div className="rounded-lg border border-border bg-bg-elev-1 p-3 space-y-1.5 mb-2">
        <div className="flex items-center justify-between mb-2">
          <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground select-none">
            {t("browser-forensics.context.connection-info", { defaultValue: "连接信息" })}
          </h4>
          <div className="flex items-center gap-2">
            <ConfidenceBadge level={overallConf} />
            <span className="text-xs text-muted-foreground select-none">
              {t("browser-forensics.context.overall-score", { defaultValue: "综合评分" })}:
            </span>
            <span className="text-xs font-mono font-semibold text-fg-primary">{overallScore}</span>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
          <InfoRow label="Domain" value={conn.domain} />
          <InfoRow label="IP" value={conn.ip ?? "-"} />
          <InfoRow label={t("browser-forensics.context.process-name", { defaultValue: "进程名" })} value={conn.process} />
          <InfoRow label={t("browser-forensics.context.pid", { defaultValue: "PID" })} value={String(conn.pid)} />
          <InfoRow label={t("browser-forensics.col.browser-profile", { defaultValue: "浏览器 / 配置文件" })} value={`${conn.browser} / ${conn.profile}`} />
          <InfoRow label={t("browser-forensics.context.connection", { defaultValue: "连接" })} value={formatTimestamp(conn.timestamp)} />
        </div>
      </div>

      {/* 历史关联 */}
      <CollapsibleSection
        title={t("browser-forensics.context.history-correlation", { defaultValue: "历史关联" })}
        defaultOpen
      >
        {hc ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <ConfidenceBadge level={hc.confidence} />
            </div>
            <ScoreBreakdown score={hc.score} />
            <div className="space-y-1.5">
              {hc.recent_activity.length === 0 ? (
                <div className="text-xs text-muted-foreground italic py-2 text-center">
                  {t("browser-forensics.context.no-recent-activity", { defaultValue: "无最近活动" })}
                </div>
              ) : (
                hc.recent_activity.map((item, idx) => {
                  const tier = item.activity.tier;
                  const color = tierColors[tier] ?? "#6b7280";
                  return (
                    <div key={idx} className="flex items-start gap-2 p-2 rounded bg-bg-elev-1 border border-border text-xs">
                      <span
                        className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider shrink-0"
                        style={{ backgroundColor: `${color}20`, color, border: `1px solid ${color}40` }}
                      >
                        {tierLabels[tier] ?? tier}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="text-fg-primary truncate font-medium" title={item.activity.title}>
                          {item.activity.title || item.activity.url}
                        </div>
                        <div className="text-[10px] text-muted-foreground truncate font-mono" title={item.activity.url}>
                          {item.activity.url}
                        </div>
                        <div className="flex gap-2 text-[10px] text-muted-foreground mt-0.5 flex-wrap">
                          <span>{formatTimestamp(item.activity.visit_time)}</span>
                          <span className="text-code">
                            {t("browser-forensics.context.score.total", { defaultValue: "总分" })}: {item.score.total}
                          </span>
                        </div>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        ) : (
          <div className="text-xs text-muted-foreground italic py-2 text-center">
            {t("browser-forensics.context.no-history-correlation", { defaultValue: "无历史关联" })}
          </div>
        )}
      </CollapsibleSection>

      {/* 导航链 */}
      <CollapsibleSection
        title={t("browser-forensics.context.navigation-chain", { defaultValue: "导航链" })}
        defaultOpen={false}
      >
        {navChain.length === 0 ? (
          <div className="text-xs text-muted-foreground italic py-4 text-center">
            {t("browser-forensics.context.no-navigation-chain", { defaultValue: "未找到导航链" })}
          </div>
        ) : (
          <div className="relative pl-6 space-y-0">
            {navChain.map((node, idx) => (
              <div key={idx} className="relative pb-4 last:pb-0">
                {idx < navChain.length - 1 && (
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
                        style={{ backgroundColor: `#6b728020`, color: "#6b7280", border: "1px solid #6b728040" }}
                      >
                        {node.transition}
                      </span>
                    )}
                    {node.qualifiers.map((q) => (
                      <span key={q} className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-accent/15 text-accent border border-accent/40">
                        {q}
                      </span>
                    ))}
                  </div>
                  <div className="text-xs text-fg-primary font-medium truncate" title={node.title ?? undefined}>
                    {node.title || "(no title)"}
                  </div>
                  <div className="text-[10px] text-muted-foreground truncate font-mono" title={node.url}>
                    {node.url}
                  </div>
                  {node.referrer && (
                    <div className="text-[10px] text-muted-foreground truncate">
                      <span className="select-none">{t("browser-forensics.context.referrer", { defaultValue: "来源" })}: </span>
                      <span className="font-mono" title={node.referrer}>{node.referrer}</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </CollapsibleSection>

      {/* 相关下载 */}
      <CollapsibleSection
        title={t("browser-forensics.context.related-downloads", { defaultValue: "相关下载记录" })}
        defaultOpen={false}
      >
        {downloads.length === 0 ? (
          <div className="text-xs text-muted-foreground italic py-2 text-center">
            {t("browser-forensics.context.no-related-downloads", { defaultValue: "无相关下载" })}
          </div>
        ) : (
          <div className="space-y-1">
            {downloads.map((d, i) => (
              <div key={i} className="flex items-start gap-2 p-1.5 rounded bg-bg-elev-1 text-xs">
                <div className="min-w-0 flex-1">
                  <div className="text-fg-primary truncate font-medium">{d.filename}</div>
                  <div className="text-[10px] text-muted-foreground truncate font-mono">{d.download_url}</div>
                  <div className="flex gap-2 text-[10px] text-muted-foreground mt-0.5 flex-wrap">
                    {d.start_time && <span>{formatTimestamp(d.start_time)}</span>}
                    {d.total_bytes != null && <span>{(d.total_bytes / 1024).toFixed(1)}KB</span>}
                    {d.danger_type !== "NOT_DANGEROUS" && (
                      <span className="text-danger">{d.danger_type}</span>
                    )}
                  </div>
                  {d.url_chain && d.url_chain.length > 1 && (
                    <div className="text-[10px] text-muted-foreground mt-0.5 truncate">
                      <span className="select-none">Chain: </span>
                      <span className="font-mono">{d.url_chain.join(" → ")}</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </CollapsibleSection>

      {/* 扩展归因 */}
      <CollapsibleSection
        title={t("browser-forensics.context.matching-extensions", { defaultValue: "匹配扩展" })}
        defaultOpen={false}
      >
        {extAttr ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <ConfidenceBadge level={extAttr.confidence} />
            </div>
            {extAttr.matched.length === 0 ? (
              <div className="text-xs text-muted-foreground italic">
                {t("browser-forensics.context.no-matching-extensions", { defaultValue: "无匹配扩展" })}
              </div>
            ) : (
              <div className="space-y-1">
                {extAttr.matched.map((ext) => (
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
            )}
          </div>
        ) : (
          <div className="text-xs text-muted-foreground italic">
            {t("browser-forensics.context.no-matching-extensions", { defaultValue: "无匹配扩展" })}
          </div>
        )}
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
      {/* Section A: 连接归因（EvidenceObject） */}
      {contextResult && <EvidenceObjectView result={contextResult} />}

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
