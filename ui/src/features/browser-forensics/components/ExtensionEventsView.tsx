import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatEpochMillis } from "@/lib/utils";
import { useBrowserForensicsStore } from "../store";
import type { AttributionLevel, ExtensionAttributionPayload } from "../types";

export function ExtensionEventsView() {
  const { t } = useTranslation();
  const extensionAttributions = useBrowserForensicsStore((s) => s.extensionAttributions);
  const clearExtensionAttributions = useBrowserForensicsStore((s) => s.clearExtensionAttributions);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState<Set<AttributionLevel>>(new Set());

  const filtered = useMemo(() => {
    return [...extensionAttributions]
      .reverse()
      .filter((evt) => {
        if (statusFilter.size > 0 && !statusFilter.has(evt.level)) return false;
        if (search.trim()) {
          const q = search.toLowerCase();
          const haystack = [evt.url, evt.initiator ?? "", evt.extension_name ?? "", evt.extension_id ?? ""].join(" ").toLowerCase();
          if (!haystack.includes(q)) return false;
        }
        return true;
      });
  }, [extensionAttributions, search, statusFilter]);

  const toggleStatus = (level: AttributionLevel) => {
    setStatusFilter((prev) => {
      const next = new Set(prev);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  };

  return (
    <div className="rounded-lg border border-border bg-bg-base">
      {/* 标题栏 + 筛选 */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border">
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground select-none">
          {t("browser-forensics.events.title", { defaultValue: "扩展事件流" })}
        </span>
        <span className="text-[10px] text-muted-foreground select-none">
          ({filtered.length}/{extensionAttributions.length})
        </span>
        <div className="flex-1" />
        {(["confirmed", "probable", "possible"] as AttributionLevel[]).map((level) => (
          <button
            key={level}
            className={`px-1.5 py-0.5 rounded text-[10px] font-medium border transition-colors ${
              statusFilter.has(level)
                ? "bg-accent/20 text-accent border-accent"
                : "bg-transparent text-muted-foreground border-border hover:border-accent/50"
            }`}
            onClick={() => toggleStatus(level)}
          >
            {t(`browser-forensics.context.confidence.${level}`, { defaultValue: level })}
          </button>
        ))}
        <Input
          className="h-7 w-40 text-xs font-mono"
          placeholder={t("browser-forensics.events.search", { defaultValue: "搜索 URL/扩展" })}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <Button
          variant="secondary"
          size="sm"
          className="h-7 text-xs"
          onClick={() => clearExtensionAttributions()}
          disabled={extensionAttributions.length === 0}
        >
          {t("common.clear", { defaultValue: "清空" })}
        </Button>
      </div>

      {/* 事件列表 */}
      <div className="max-h-80 overflow-y-auto">
        {filtered.length === 0 ? (
          <div className="px-3 py-4 text-xs text-muted-foreground italic text-center">
            {extensionAttributions.length === 0
              ? t("browser-forensics.events.empty", { defaultValue: "等待 Helper Extension 上报事件" })
              : t("browser-forensics.events.no-match", { defaultValue: "无匹配事件" })}
          </div>
        ) : (
          <div className="divide-y divide-border">
            {filtered.map((evt, idx) => (
              <ExtensionEventCard key={`${evt.request_id}-${evt.timestamp}-${idx}`} evt={evt} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function formatTs(ts: number): string {
  return formatEpochMillis(ts);
}

/// 将 URL 分解为 { protocol, host, path, query }，用于分层高亮展示
function parseUrlParts(url: string): { host: string; rest: string } | null {
  try {
    const u = new URL(url);
    const host = u.host;
    let rest = u.pathname;
    if (u.search) rest += u.search;
    if (u.hash) rest += u.hash;
    return { host, rest };
  } catch {
    return null;
  }
}

function ExtensionEventCard({ evt }: { evt: ExtensionAttributionPayload }) {
  const { t } = useTranslation();
  const levelStyles: Record<AttributionLevel, string> = {
    confirmed: "bg-success/15 text-success border-success/40",
    probable: "bg-warning/15 text-warning border-warning/40",
    possible: "bg-muted/30 text-muted-foreground border-border",
  };
  const levelLabels: Record<AttributionLevel, string> = {
    confirmed: t("browser-forensics.context.confidence.confirmed", { defaultValue: "已确认" }),
    probable: t("browser-forensics.context.confidence.probable", { defaultValue: "较可能" }),
    possible: t("browser-forensics.context.confidence.possible", { defaultValue: "可能" }),
  };

  // 源头信息：extension_name 字段在 CDP 路径下存放 target URL（如 chrome-extension://<id>/background.js）
  // 用于体现"请求是由什么发起的"
  const sourceLabel = evt.extension_name;
  const sourceTitle = evt.extension_id ? `Extension ID: ${evt.extension_id}` : undefined;

  // CDP 路径独有字段：target_type（page/service_worker/background_page）+ initiator_type（script/parser/...）
  // 用于区分请求来自"页面""扩展 SW"还是"background_page"，以及触发方式
  const targetTypeLabel = (() => {
    if (!evt.target_type) return null;
    switch (evt.target_type) {
      case "page": return "页面";
      case "service_worker": return "扩展 SW";
      case "background_page": return "BG Page";
      default: return evt.target_type;
    }
  })();
  const targetTypeStyle = evt.target_type === "service_worker" || evt.target_type === "background_page"
    ? "bg-accent/15 text-accent border-accent/40"
    : "bg-muted/30 text-muted-foreground border-border";

  // initiator 友好化：chrome-error://chromewebdata/ → "Chrome 错误页"
  const initiatorLabel = (() => {
    if (!evt.initiator) return null;
    if (evt.initiator === "chrome-error://chromewebdata/") {
      return t("browser-forensics.events.initiator-chrome-error", { defaultValue: "Chrome 错误页" });
    }
    if (evt.initiator.startsWith("chrome-extension://")) {
      const path = evt.initiator.slice("chrome-extension://".length);
      return `extension://${path}`;
    }
    return evt.initiator;
  })();

  return (
    <div className="px-3 py-1.5 hover:bg-bg-elev-1/50 transition-colors">
      <div className="flex items-center gap-2 flex-wrap">
        <span className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider border ${levelStyles[evt.level]}`}>
          {levelLabels[evt.level]}
        </span>
        {targetTypeLabel && (
          <span
            className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium border ${targetTypeStyle} select-none`}
            title={evt.target_title ?? undefined}
          >
            {targetTypeLabel}
            {evt.target_title && <span className="ml-1 opacity-70 truncate max-w-[200px]">{evt.target_title}</span>}
          </span>
        )}
        <span className="text-[10px] text-muted-foreground font-mono select-none">{evt.method}</span>
        {evt.resource_type && (
          <span className="text-[10px] text-muted-foreground font-mono px-1 rounded border border-border select-none">
            {evt.resource_type}
          </span>
        )}
        <span className="text-[10px] text-muted-foreground font-mono">{formatTs(evt.timestamp)}</span>
        <div className="flex-1" />
        {sourceLabel && (
          <span className="text-[10px] text-accent font-mono" title={sourceTitle}>
            {sourceLabel}
          </span>
        )}
      </div>
      {/* URL：分层展示——host 高亮，path 淡化，完整不截断 */}
      <div className="mt-0.5 text-xs break-all font-mono">
        {(() => {
          const parts = parseUrlParts(evt.url);
          if (parts) {
            return (
              <>
                <span className="text-muted-foreground">{new URL(evt.url).protocol}//</span>
                <span className="text-fg-primary font-semibold">{parts.host}</span>
                <span className="text-muted-foreground">{parts.rest}</span>
              </>
            );
          }
          return <span className="text-fg-primary">{evt.url}</span>;
        })()}
      </div>
      {initiatorLabel && (
        <div className="mt-0.5 text-[10px] text-muted-foreground break-all font-mono flex items-center gap-1.5 flex-wrap">
          {evt.initiator_type && (
            <span className="px-1 rounded border border-border text-muted-foreground select-none">
              {evt.initiator_type}
            </span>
          )}
          <span>
            {t("browser-forensics.events.initiator-label", { defaultValue: "源头" })}: {initiatorLabel}
          </span>
        </div>
      )}
    </div>
  );
}
