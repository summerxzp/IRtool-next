import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Play, Square, History, Trash2, Download, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS } from "../types";
import type { SysmonEventType } from "../types";

interface Props {
  onStart: () => void;
  onStop: () => void;
  onLoadHistory: () => void;
  onClear: () => void;
  onExport: () => void;
  collecting: boolean;
  loading: boolean;
}

const EVENT_TYPE_OPTIONS: Array<SysmonEventType | "all"> = ["all", "dns", "network_connect", "create_remote_thread", "file_create"];

export function LogCollectorToolbar({ onStart, onStop, onLoadHistory, onClear, onExport, collecting, loading }: Props) {
  const { t } = useTranslation();
  const { filters, setFilter } = useLogCollectorStore();
  const [searchInput, setSearchInput] = useState(filters.search);

  useEffect(() => {
    const id = setTimeout(() => setFilter("search", searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setFilter]);

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      {collecting ? (
        <Button variant="destructive" size="sm" onClick={onStop}>
          <Square className="h-3.5 w-3.5" />
          <span className="ml-1">{t("log-collector.toolbar.stop")}</span>
        </Button>
      ) : (
        <Button variant="default" size="sm" onClick={onStart} className="bg-green-600 hover:bg-green-700">
          <Play className="h-3.5 w-3.5" />
          <span className="ml-1">{t("log-collector.toolbar.start")}</span>
        </Button>
      )}

      <Button variant="secondary" size="sm" onClick={onLoadHistory} disabled={collecting || loading}>
        <History className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.load-history")}
      </Button>

      <Select value={filters.eventType} onValueChange={(v) => setFilter("eventType", v as SysmonEventType | "all")}>
        <SelectTrigger className="h-7 w-28 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {EVENT_TYPE_OPTIONS.map((et) => (
            <SelectItem key={et} value={et}>
              {et === "all" ? t("log-collector.toolbar.all-events") : EVENT_TYPE_LABELS[et]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <div className="flex items-center gap-1.5">
        <Checkbox
          id="external-only"
          checked={filters.externalOnly}
          onCheckedChange={(v) => setFilter("externalOnly", v === true)}
        />
        <Label htmlFor="external-only" className="text-xs cursor-pointer">
          {t("log-collector.toolbar.external-only")}
        </Label>
      </div>

      <Input
        type="text"
        placeholder={t("log-collector.toolbar.search-placeholder")}
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        className="flex-1 max-w-xs"
      />
      {searchInput && (
        <Button variant="ghost" size="icon" onClick={() => setSearchInput("")} title="clear">
          <X className="h-3.5 w-3.5" />
        </Button>
      )}

      <div className="flex-1" />

      <Button variant="secondary" size="sm" onClick={onExport} disabled={loading}>
        <Download className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.export")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onClear} disabled={loading}>
        <Trash2 className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.clear")}
      </Button>
    </div>
  );
}
