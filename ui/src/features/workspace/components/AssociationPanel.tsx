import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight, Link } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { useWorkspaceStore } from "../store";
import { findAssociations } from "../association";
import type { WorkspaceTab } from "../types";
import { useState, useMemo } from "react";

interface Props {
  sourceTab: WorkspaceTab;
  sourceItem: object | null;
}

export function AssociationPanel({ sourceTab, sourceItem }: Props) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState(false);
  const setActiveTab = useWorkspaceStore((s) => s.setActiveTab);
  const autorunItems = useWorkspaceStore((s) => s.autorunItems);
  const networkItems = useWorkspaceStore((s) => s.networkItems);
  const eventItems = useWorkspaceStore((s) => s.eventItems);

  const result = useMemo(() => {
    if (!sourceItem) return null;
    return findAssociations(
      sourceTab,
      sourceItem as any,
      autorunItems,
      networkItems,
      eventItems,
    );
  }, [sourceTab, sourceItem, autorunItems, networkItems, eventItems]);

  if (!result || !sourceItem) {
    return (
      <div className="px-3 py-2 text-xs text-fg-tertiary border-t border-border">
        {t("workspace.association.no-results")}
      </div>
    );
  }

  const groups = [
    { tab: "autoruns" as WorkspaceTab, label: t("workspace.tabs.autoruns"), items: result.autoruns },
    { tab: "network" as WorkspaceTab, label: t("workspace.tabs.network"), items: result.network },
    { tab: "events" as WorkspaceTab, label: t("workspace.tabs.events"), items: result.events },
  ].filter((g) => g.items.length > 0);

  if (groups.length === 0) {
    return (
      <div className="px-3 py-2 text-xs text-fg-tertiary border-t border-border">
        {t("workspace.association.no-results")}
      </div>
    );
  }

  return (
    <div className="border-t border-border">
      <button
        className="flex items-center gap-1.5 w-full px-3 py-1.5 text-xs text-fg-secondary hover:bg-bg-elev-2/40"
        onClick={() => setCollapsed(!collapsed)}
      >
        {collapsed ? <ChevronRight className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
        <Link className="h-3 w-3" />
        <span>{t("workspace.association.key")}: {result.sourceKey}</span>
      </button>
      {!collapsed && (
        <div className="px-3 pb-2 flex flex-wrap gap-2">
          {groups.map((g) => (
            <button
              key={g.tab}
              className="flex items-center gap-1 px-2 py-1 text-xs rounded-md bg-bg-elev-2 hover:bg-accent/10 transition-colors"
              onClick={() => setActiveTab(g.tab)}
            >
              {g.label}
              <Badge variant="info" className="text-[10px] px-1 py-0">{g.items.length}</Badge>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
