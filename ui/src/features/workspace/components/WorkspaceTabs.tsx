import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { useWorkspaceStore } from "../store";
import type { WorkspaceTab } from "../types";

interface Props {
  autorunCount: number;
  networkCount: number;
  eventCount: number;
}

export function WorkspaceTabs({ autorunCount, networkCount, eventCount }: Props) {
  const { t } = useTranslation();
  const activeTab = useWorkspaceStore((s) => s.activeTab);
  const setActiveTab = useWorkspaceStore((s) => s.setActiveTab);

  const tabs: { key: WorkspaceTab; label: string; count: number }[] = [
    { key: "autoruns", label: t("workspace.tabs.autoruns"), count: autorunCount },
    { key: "network", label: t("workspace.tabs.network"), count: networkCount },
    { key: "events", label: t("workspace.tabs.events"), count: eventCount },
  ];

  return (
    <div className="flex items-center gap-1 px-2 py-1 bg-bg-elev-1 border-b border-border">
      {tabs.map((tab) => (
        <button
          key={tab.key}
          className={`flex items-center gap-1.5 px-3 py-1 text-sm rounded-md transition-colors ${
            activeTab === tab.key
              ? "bg-accent/15 text-accent font-medium"
              : "text-fg-secondary hover:text-fg-primary hover:bg-bg-elev-2"
          }`}
          onClick={() => setActiveTab(tab.key)}
        >
          {tab.label}
          {tab.count > 0 && (
            <Badge variant={activeTab === tab.key ? "info" : "default"} className="text-[10px] px-1 py-0">
              {tab.count}
            </Badge>
          )}
        </button>
      ))}
    </div>
  );
}
