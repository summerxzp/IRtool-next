import { useState } from "react";
import { Link, useRouterState } from "@tanstack/react-router";
import { Activity, ScrollText, Repeat, Briefcase, Settings, Database, Radar, PanelLeftClose, PanelLeft, Cpu, Globe } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { useMonitoringStore } from "@/features/monitoring/store";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface NavItem {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  i18nKey: string;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/network", icon: Activity, i18nKey: "nav.network" },
  { to: "/log-collector", icon: ScrollText, i18nKey: "nav.log-collector" },
  { to: "/autoruns", icon: Repeat, i18nKey: "nav.autoruns" },
  { to: "/process", icon: Cpu, i18nKey: "nav.process" },
  { to: "/browser-forensics", icon: Globe, i18nKey: "nav.browser-forensics" },
  { to: "/workspace", icon: Briefcase, i18nKey: "nav.workspace" },
];

const BOTTOM_ITEMS: NavItem[] = [
  { to: "/background-monitoring", icon: Radar, i18nKey: "nav.background-monitoring" },
  { to: "/database-search", icon: Database, i18nKey: "nav.database-search" },
  { to: "/settings", icon: Settings, i18nKey: "nav.settings" },
];

export function Sidebar() {
  const { t } = useTranslation();
  const path = useRouterState({ select: (s) => s.location.pathname });
  const isBackground = useMonitoringStore((s) => s.isBackground);
  const [expanded, setExpanded] = useState(() =>
    localStorage.getItem("sidebar-expanded") === "true"
  );

  const renderItem = (item: NavItem, extraClass?: string) => {
    const Icon = item.icon;
    const isActive = path.startsWith(item.to);
    return (
      <Tooltip key={item.to}>
        <TooltipTrigger asChild>
          <Link
            to={item.to}
            className={cn(
              "h-10 rounded-md flex items-center justify-center transition-colors relative",
              expanded ? "w-full px-2 gap-2" : "w-10",
              isActive
                ? "text-accent"
                : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
              extraClass,
            )}
          >
            {isActive && (
              <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
            )}
            <Icon className="h-5 w-5 shrink-0" />
            {expanded && (
              <span className="text-xs whitespace-nowrap select-none">{t(item.i18nKey)}</span>
            )}
          </Link>
        </TooltipTrigger>
        {!expanded && (
          <TooltipContent side="right">
            {t(item.i18nKey)}
          </TooltipContent>
        )}
      </Tooltip>
    );
  };

  return (
    <TooltipProvider delayDuration={300}>
      <aside className={cn(
        "bg-bg-elev-1 border-r border-border flex flex-col transition-all duration-200",
        expanded ? "w-40" : "w-14",
      )}>
        {/* 展开/收起按钮 */}
        <div className="pt-2 pb-1 flex justify-center">
          <button
            onClick={() => {
              const next = !expanded;
              setExpanded(next);
              localStorage.setItem("sidebar-expanded", String(next));
            }}
            className="h-8 w-8 rounded-md flex items-center justify-center text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2 transition-colors"
          >
            {expanded ? <PanelLeftClose className="h-4 w-4" /> : <PanelLeft className="h-4 w-4" />}
          </button>
        </div>
        <div className="flex-1 flex flex-col items-center gap-1 pt-1">
          {NAV_ITEMS.map((item) => renderItem(item))}
        </div>
        <div className="pb-3 flex flex-col items-center gap-1">
          {BOTTOM_ITEMS.map((item) =>
            renderItem(item, item.to === "/background-monitoring" && isBackground ? "border-2 border-red-500" : undefined),
          )}
        </div>
      </aside>
    </TooltipProvider>
  );
}
