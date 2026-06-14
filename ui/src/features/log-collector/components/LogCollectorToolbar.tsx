import { useEffect, useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Play, Square, Trash2, Download, X, Settings, Unplug, ChevronDown, History } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useLogCollectorStore } from "../store";
import { EVENT_TYPE_LABELS } from "../types";
import type { ExtendedSysmonEventType, SysmonEvent } from "../types";

interface Props {
  onStart: () => void;
  onStop: () => void;
  onLoadHistory: () => void;
  onClear: () => void;
  onExport: () => void;
  onUninstall: () => void;
  onOpenConfigDialog: () => void;
  collecting: boolean;
  loading: boolean;
  sysmonInstalled: boolean;
  events: SysmonEvent[];
}

export function LogCollectorToolbar({ onStart, onStop, onLoadHistory, onClear, onExport, onUninstall, onOpenConfigDialog, collecting, loading, sysmonInstalled, events }: Props) {
  const { t } = useTranslation();
  const { filters, setFilter } = useLogCollectorStore();
  const [searchInput, setSearchInput] = useState(filters.search);

  // Dynamically build event type options from existing events
  const eventTypeOptions = useMemo(() => {
    const types = new Set<ExtendedSysmonEventType>();
    for (const e of events) {
      if (e.event_type !== "unknown") types.add(e.event_type);
    }
    return Array.from(types).sort();
  }, [events]);

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

      <Button variant="secondary" size="sm" onClick={onLoadHistory} disabled={loading} className="hover:shadow-sm transition-shadow">
        <History className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.load-history")}
      </Button>

      <Popover>
        <PopoverTrigger asChild>
          <Button variant="secondary" size="sm" className="h-7 text-xs">
            {filters.eventTypes.length === 0 ? t("log-collector.toolbar.all-events") : `${t("log-collector.toolbar.event-type-label")} (${filters.eventTypes.length})`}
            <ChevronDown className="h-3 w-3 ml-1" />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-48 p-2 max-h-60 overflow-y-auto" align="start">
          {eventTypeOptions.map((et) => (
            <div
              key={et}
              className="flex items-center gap-2 py-0.5 cursor-pointer hover:bg-bg-elev-2 rounded px-1"
              onClick={() => {
                const next = filters.eventTypes.includes(et)
                  ? filters.eventTypes.filter((v) => v !== et)
                  : [...filters.eventTypes, et];
                setFilter("eventTypes", next);
              }}
            >
              <Checkbox checked={filters.eventTypes.includes(et)} />
              <Label className="text-xs cursor-pointer">{EVENT_TYPE_LABELS[et]}</Label>
            </div>
          ))}
        </PopoverContent>
      </Popover>

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

      <Button variant="secondary" size="sm" onClick={onOpenConfigDialog} disabled={loading} className="hover:shadow-sm transition-shadow">
        <Settings className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.config")}
      </Button>
      {sysmonInstalled && (
        <Button variant="secondary" size="sm" onClick={onUninstall} disabled={loading} title={t("log-collector.toolbar.uninstall")} className="hover:shadow-sm transition-shadow">
          <Unplug className="h-3.5 w-3.5 mr-1" />
          {t("log-collector.toolbar.uninstall")}
        </Button>
      )}
      <Button variant="secondary" size="sm" onClick={onExport} disabled={loading} className="hover:shadow-sm transition-shadow">
        <Download className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.export")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onClear} disabled={loading} className="hover:shadow-sm transition-shadow">
        <Trash2 className="h-3.5 w-3.5 mr-1" />
        {t("log-collector.toolbar.clear")}
      </Button>
    </div>
  );
}
