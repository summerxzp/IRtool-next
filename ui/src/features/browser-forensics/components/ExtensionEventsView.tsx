import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
  const d = new Date(ts);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const h = String(d.getHours()).padStart(2, "0");
  const min = String(d.getMinutes()).padStart(2, "0");
  const s = String(d.getSeconds()).padStart(2, "0");
  return `${y}/${m}/${day} ${h}:${min}:${s}`;
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

  return (
    <div className="px-3 py-1.5 hover:bg-bg-elev-1/50 transition-colors">
      <div className="flex items-center gap-2">
        <span className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wider border ${levelStyles[evt.level]}`}>
          {levelLabels[evt.level]}
        </span>
        <span className="text-[10px] text-muted-foreground font-mono select-none">{evt.method}</span>
        {evt.resource_type && (
          <span className="text-[10px] text-muted-foreground font-mono px-1 rounded border border-border select-none">
            {evt.resource_type}
          </span>
        )}
        <span className="text-[10px] text-muted-foreground font-mono">{formatTs(evt.timestamp)}</span>
        <div className="flex-1" />
        {evt.extension_name && (
          <span className="text-[10px] text-accent truncate max-w-32" title={evt.extension_id ?? ""}>
            {evt.extension_name}
          </span>
        )}
      </div>
      <div className="mt-0.5 text-xs text-fg-primary truncate font-mono" title={evt.url}>
        {evt.url}
      </div>
      {evt.initiator && (
        <div className="mt-0.5 text-[10px] text-muted-foreground truncate font-mono">
          initiator: {evt.initiator}
        </div>
      )}
    </div>
  );
}
