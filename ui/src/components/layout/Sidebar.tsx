import { Link, useRouterState } from "@tanstack/react-router";
import { Activity, ScrollText, Repeat, Briefcase, Settings, Database, Radar } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
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
  { to: "/workspace", icon: Briefcase, i18nKey: "nav.workspace" },
];

export function Sidebar() {
  const { t } = useTranslation();
  const path = useRouterState({ select: (s) => s.location.pathname });

  return (
    <TooltipProvider delayDuration={300}>
      <aside className="w-14 bg-bg-elev-1 border-r border-border flex flex-col">
        <div className="flex-1 flex flex-col items-center pt-3 gap-1">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const isActive = path.startsWith(item.to);
            return (
              <Tooltip key={item.to}>
                <TooltipTrigger asChild>
                  <Link
                    to={item.to}
                    className={cn(
                      "h-10 w-10 rounded-md flex items-center justify-center transition-colors relative",
                      isActive
                        ? "text-accent"
                        : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                    )}
                  >
                    {isActive && (
                      <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
                    )}
                    <Icon className="h-5 w-5" />
                  </Link>
                </TooltipTrigger>
                <TooltipContent side="right">
                  {t(item.i18nKey)}
                </TooltipContent>
              </Tooltip>
            );
          })}
        </div>
        <div className="pb-3 flex flex-col items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Link
                to="/background-monitoring"
                className={cn(
                  "h-10 w-10 rounded-md flex items-center justify-center transition-colors relative",
                  path.startsWith("/background-monitoring")
                    ? "text-accent"
                    : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                )}
              >
                {path.startsWith("/background-monitoring") && (
                  <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
                )}
                <Radar className="h-5 w-5" />
              </Link>
            </TooltipTrigger>
            <TooltipContent side="right">{t("nav.background-monitoring")}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Link
                to="/database-search"
                className={cn(
                  "h-10 w-10 rounded-md flex items-center justify-center transition-colors relative",
                  path.startsWith("/database-search")
                    ? "text-accent"
                    : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                )}
              >
                {path.startsWith("/database-search") && (
                  <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
                )}
                <Database className="h-5 w-5" />
              </Link>
            </TooltipTrigger>
            <TooltipContent side="right">数据库检索</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Link
                to="/settings"
                className={cn(
                  "h-10 w-10 rounded-md flex items-center justify-center transition-colors relative",
                  path.startsWith("/settings")
                    ? "text-accent"
                    : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                )}
              >
                {path.startsWith("/settings") && (
                  <span className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-accent rounded-r" />
                )}
                <Settings className="h-5 w-5" />
              </Link>
            </TooltipTrigger>
            <TooltipContent side="right">{t("nav.settings")}</TooltipContent>
          </Tooltip>
        </div>
      </aside>
    </TooltipProvider>
  );
}
