import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, List, GitBranch, X, ChevronsUpDown, Play, Pause } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useProcessStore } from "../store";
import type { FilterMode } from "../types";

interface Props {
  onRefresh: () => void;
  loading: boolean;
  snapshotTime: string | null;
}

const FILTER_OPTIONS: FilterMode[] = ["all", "suspicious"];
const INTERVAL_OPTIONS = [
  { value: 1000, label: "1s" },
  { value: 2000, label: "2s" },
  { value: 5000, label: "5s" },
  { value: 10000, label: "10s" },
];

export function ProcessToolbar({ onRefresh, loading, snapshotTime }: Props) {
  const { t } = useTranslation();
  const viewMode = useProcessStore((s) => s.viewMode);
  const setViewMode = useProcessStore((s) => s.setViewMode);
  const filter = useProcessStore((s) => s.filter);
  const setFilter = useProcessStore((s) => s.setFilter);
  const search = useProcessStore((s) => s.search);
  const setSearch = useProcessStore((s) => s.setSearch);
  const toggleExpandAll = useProcessStore((s) => s.toggleExpandAll);
  const expandAllCollapsed = useProcessStore((s) => s.expandAllCollapsed);
  const autoRefreshMs = useProcessStore((s) => s.autoRefreshMs);
  const setAutoRefreshMs = useProcessStore((s) => s.setAutoRefreshMs);

  const isAutoRefresh = autoRefreshMs > 0;

  const [searchInput, setSearchInput] = useState(search);
  useEffect(() => {
    const id = setTimeout(() => setSearch(searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setSearch]);

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      <Button variant="default" size="sm" onClick={onRefresh} disabled={loading} className="hover:shadow-sm transition-shadow">
        <RefreshCw className={`h-3.5 w-3.5 mr-1 ${loading ? "animate-spin" : ""}`} />{t("process.toolbar.refresh")}
      </Button>

      <Button
        variant={isAutoRefresh ? "secondary" : "ghost"}
        size="sm"
        className="h-7"
        onClick={() => setAutoRefreshMs(isAutoRefresh ? 0 : 1000)}
      >
        {isAutoRefresh ? <Pause className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
        <span className="ml-1">{isAutoRefresh ? t("process.toolbar.pause") : t("process.toolbar.auto")}</span>
      </Button>

      {isAutoRefresh && (
        <Select value={String(autoRefreshMs)} onValueChange={(v) => setAutoRefreshMs(parseInt(v, 10))}>
          <SelectTrigger className="h-7 w-16 text-xs"><SelectValue /></SelectTrigger>
          <SelectContent>
            {INTERVAL_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={String(opt.value)}>{opt.label}</SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}

      <div className="flex items-center bg-bg-elev-2 rounded-md p-0.5">
        <Button variant={viewMode === "list" ? "secondary" : "ghost"} size="sm" className="h-6 px-2" onClick={() => setViewMode("list")}>
          <List className="h-3.5 w-3.5 mr-1" />{t("process.toolbar.view.list")}
        </Button>
        <Button variant={viewMode === "tree" ? "secondary" : "ghost"} size="sm" className="h-6 px-2" onClick={() => setViewMode("tree")}>
          <GitBranch className="h-3.5 w-3.5 mr-1" />{t("process.toolbar.view.tree")}
        </Button>
      </div>

      {viewMode === "tree" && (
        <Button variant="ghost" size="sm" className="h-6 px-2" onClick={toggleExpandAll} title={expandAllCollapsed ? t("process.toolbar.expand-all") : t("process.toolbar.collapse-all")}>
          <ChevronsUpDown className="h-3.5 w-3.5 mr-1" />
          {expandAllCollapsed ? t("process.toolbar.expand-all") : t("process.toolbar.collapse-all")}
        </Button>
      )}

      <Select value={filter} onValueChange={(v) => setFilter(v as FilterMode)}>
        <SelectTrigger className="h-7 w-24 text-xs"><SelectValue /></SelectTrigger>
        <SelectContent>
          {FILTER_OPTIONS.map((f) => (<SelectItem key={f} value={f}>{t(`process.toolbar.filter.${f}`)}</SelectItem>))}
        </SelectContent>
      </Select>

      <Input type="text" placeholder={t("process.toolbar.search-placeholder")} value={searchInput} onChange={(e) => setSearchInput(e.target.value)} className="flex-1 min-w-0" />
      {searchInput && (<Button variant="ghost" size="icon" onClick={() => setSearchInput("")}><X className="h-3.5 w-3.5" /></Button>)}

      {snapshotTime && (
        <span className="text-xs text-fg-tertiary select-none">{snapshotTime}</span>
      )}
    </div>
  );
}
