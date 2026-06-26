import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { HistoryAttribution, RecentActivity, NavChainNode } from "../types";
import { formatTimestamp } from "../utils";

// ── 颜色常量 ─────────────────────────────────────────────────────

const TIER_COLORS: Record<string, string> = {
  immediate: "#ef4444",
  nearby: "#eab308",
  recent: "#6b7280",
};

const TRANSITION_COLORS: Record<string, string> = {
  LINK: "#3b82f6",
  REDIRECT: "#f97316",
  TYPED: "#22c55e",
  AUTO_BOOKMARK: "#a855f7",
};

function getTransitionColor(transition: string | null): string {
  if (!transition) return "#6b7280";
  return TRANSITION_COLORS[transition] ?? "#6b7280";
}

// ── 子组件 ──────────────────────────────────────────────────────

function formatTimeDistance(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  return `${Math.floor(ms / 3600000)}h ${Math.floor((ms % 3600000) / 60000)}m`;
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
  const { t } = useTranslation();
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
        <div className="flex gap-2 text-[10px] text-muted-foreground mt-0.5 flex-wrap">
          <span>{formatTimestamp(activity.visit_time)}</span>
          <span className="select-none">
            {t("browser-forensics.col.tier", { defaultValue: "时间层" })}: {formatTimeDistance(activity.time_distance_ms)}
          </span>
          {activity.score && (
            <span className="text-code select-none">
              {t("browser-forensics.context.score.total", { defaultValue: "总分" })}: {activity.score.total}
            </span>
          )}
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
        <span
          className="w-2.5 h-2.5 rounded-full inline-block"
          style={{ backgroundColor: color }}
        />
        <span className="text-xs font-semibold uppercase tracking-wider" style={{ color }}>
          {labels[tier] ?? tier}
        </span>
        <Badge variant="outline" className="text-[10px]">{activities.length}</Badge>
      </div>
      <div className="space-y-1.5">
        {activities.map((activity, idx) => (
          <ActivityCard key={`${activity.url}-${activity.visit_time}-${idx}`} activity={activity} />
        ))}
      </div>
    </div>
  );
}

function TransitionBadge({ transition }: { transition: string | null }) {
  const color = getTransitionColor(transition);
  const label = transition ?? "UNKNOWN";
  return (
    <span
      className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono font-medium"
      style={{
        backgroundColor: `${color}20`,
        color,
        border: `1px solid ${color}40`,
      }}
    >
      {label}
    </span>
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
          {/* 竖线 */}
          {idx < nodes.length - 1 && (
            <div className="absolute left-[7px] top-3 bottom-0 w-0.5 bg-border" />
          )}
          {/* 圆点 */}
          <div className="absolute left-0 top-1.5 w-[15px] flex items-center justify-center">
            <div className="w-2.5 h-2.5 rounded-full border-2 border-accent bg-bg-base" />
          </div>
          {/* 内容 */}
          <div className="ml-3 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {node.transition && <TransitionBadge transition={node.transition} />}
              {node.qualifiers.map((q) => (
                <span key={q} className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-mono bg-accent/15 text-accent border border-accent/40">
                  {q}
                </span>
              ))}
            </div>
            <div className="text-xs text-fg-primary font-medium truncate" title={node.title ?? undefined}>
              {node.title || t("browser-forensics.context.no-title", { defaultValue: "(无标题)" })}
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
  );
}

// ── 主组件 ──────────────────────────────────────────────────────

interface AttributionHistoryViewProps {
  attribution: HistoryAttribution | null;
  loading: boolean;
}

export function AttributionHistoryView({ attribution, loading }: AttributionHistoryViewProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div className="flex items-center justify-center py-8 gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        <span>{t("common.scanning", { defaultValue: "正在分析..." })}</span>
      </div>
    );
  }

  if (!attribution) return null;

  // 按 tier 分组
  const grouped: Record<string, RecentActivity[]> = {};
  for (const activity of attribution.recent_browser_activity) {
    const tier = activity.tier;
    if (!grouped[tier]) grouped[tier] = [];
    grouped[tier].push(activity);
  }

  const tierOrder = ["immediate", "nearby", "recent"];

  return (
    <div className="space-y-4 p-3">
      {/* Profile 信息 */}
      <div className="text-[10px] text-muted-foreground uppercase tracking-wider font-semibold">
        {attribution.browser} / {attribution.profile}
      </div>

      {/* Section A: 分层活动 */}
      <div className="space-y-3">
        {tierOrder.map((tier) => (
          <TierGroup key={tier} tier={tier} activities={grouped[tier] ?? []} />
        ))}
        {attribution.recent_browser_activity.length === 0 && (
          <div className="text-xs text-muted-foreground italic py-2 text-center">
            {t("browser-forensics.context.no-recent-activity", { defaultValue: "无最近活动" })}
          </div>
        )}
      </div>

      {/* Divider */}
      <div className="border-t border-border" />

      {/* Section B: 导航链 */}
      <div>
        <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
          {t("browser-forensics.context.navigation-chain", { defaultValue: "导航链" })}
        </h3>
        <NavChainTimeline nodes={attribution.navigation_chain} />
      </div>
    </div>
  );
}
