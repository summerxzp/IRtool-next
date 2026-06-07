import { Link, useRouterState } from "@tanstack/react-router";
import { Activity, ScrollText, Repeat, Briefcase, Settings, Eye, Database } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

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
  const [isBackground, setIsBackground] = useState(false);
  const [confirmDialogOpen, setConfirmDialogOpen] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const bg = await invoke<boolean>("cmd_monitor_is_background");
        setIsBackground(bg);
      } catch {}
    })();
  }, []);

  const toggleBackground = useCallback(async () => {
    try {
      if (isBackground) {
        await invoke("cmd_monitor_exit_background");
        setIsBackground(false);
      } else {
        setConfirmDialogOpen(true);
      }
    } catch {}
  }, [isBackground]);

  const confirmEnterBackground = useCallback(async () => {
    try {
      await invoke("cmd_monitor_enter_background");
      setIsBackground(true);
      setConfirmDialogOpen(false);
    } catch {}
  }, []);

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
              <button
                onClick={toggleBackground}
                className={cn(
                  "h-10 w-10 rounded-md flex items-center justify-center transition-colors",
                  isBackground
                    ? "text-accent bg-accent/10"
                    : "text-fg-tertiary hover:text-fg-primary hover:bg-bg-elev-2",
                )}
              >
                <Eye className="h-5 w-5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t("nav.background-detection")}</TooltipContent>
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

      <Dialog open={confirmDialogOpen} onOpenChange={setConfirmDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>进入后台监控模式</DialogTitle>
            <DialogDescription>
              进入后台模式后，主窗口将隐藏到托盘，前端不会实时显示新事件，但数据采集和告警功能会继续运行，事件会持久化到 SQLite 数据库中。
              <br /><br />
              点击托盘图标可以恢复窗口查看数据。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setConfirmDialogOpen(false)}>取消</Button>
            <Button onClick={confirmEnterBackground}>确认进入</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </TooltipProvider>
  );
}
