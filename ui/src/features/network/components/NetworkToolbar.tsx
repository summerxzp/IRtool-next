import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pause, Play, RefreshCcw, Trash2, Download, X, ChevronDown } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useNetworkStore } from "../store";
import type { ConnState, Proto, RetentionPolicyDto } from "../types";

interface Props {
  onExport: () => void;
  onClearHistory: () => void;
  onRefresh: () => void;
  onKillSelected: () => void;
  hasSelection: boolean;
  loading: boolean;
}

const PROTO_OPTIONS: Array<Proto | "all"> = ["all", "tcp", "udp"];
const STATE_OPTIONS: ConnState[] = [
  "ESTABLISHED",
  "LISTEN",
  "TIME_WAIT",
  "CLOSE_WAIT",
  "SYN_SENT",
  "SYN_RCVD",
];

export function NetworkToolbar({
  onExport,
  onClearHistory,
  onKillSelected,
  hasSelection,
  loading,
}: Props) {
  const { t } = useTranslation();
  const {
    filters,
    setFilter,
    paused,
    setPaused,
    intervalMs,
    setIntervalMs,
    retention,
    setRetention,
  } = useNetworkStore();

  const [searchInput, setSearchInput] = useState(filters.search);
  useEffect(() => {
    const id = setTimeout(() => setFilter("search", searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setFilter]);

  const retentionValue =
    retention === "forever"
      ? "forever"
      : retention === "none"
        ? "none"
        : `s${(retention as { seconds: number }).seconds}`;

  const handleRetentionChange = (v: string) => {
    let next: RetentionPolicyDto;
    if (v === "forever") next = "forever";
    else if (v === "none") next = "none";
    else next = { seconds: parseInt(v.replace("s", ""), 10) };
    setRetention(next);
  };

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      <Button
        variant={paused ? "secondary" : "default"}
        size="sm"
        onClick={() => setPaused(!paused)}
        className="hover:shadow-sm transition-shadow"
      >
        {paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
        <span className="ml-1">
          {paused ? t("network.toolbar.resume") : t("network.toolbar.pause")}
        </span>
      </Button>

      <Select value={String(intervalMs)} onValueChange={(v) => setIntervalMs(parseInt(v, 10))}>
        <SelectTrigger className="h-7 w-24 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="1000">{t("network.toolbar.interval-1s")}</SelectItem>
          <SelectItem value="2000">{t("network.toolbar.interval-2s")}</SelectItem>
          <SelectItem value="5000">{t("network.toolbar.interval-5s")}</SelectItem>
        </SelectContent>
      </Select>

      <Select
        value={filters.proto}
        onValueChange={(v) => setFilter("proto", v as Proto | "all")}
      >
        <SelectTrigger className="h-7 w-24 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {PROTO_OPTIONS.map((p) => (
            <SelectItem key={p} value={p}>
              {p === "all" ? t("network.toolbar.proto-all") : p.toUpperCase()}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Popover>
        <PopoverTrigger asChild>
          <Button variant="secondary" size="sm" className="h-7 text-xs">
            {filters.states.length === 0 ? t("network.toolbar.state-all") : `${t("network.toolbar.state-label")} (${filters.states.length})`}
            <ChevronDown className="h-3 w-3 ml-1" />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-40 p-2" align="start">
          {STATE_OPTIONS.map((s) => (
            <div
              key={s}
              className="flex items-center gap-2 py-0.5 cursor-pointer hover:bg-bg-elev-2 rounded px-1"
              onClick={() => {
                const next = filters.states.includes(s)
                  ? filters.states.filter((v) => v !== s)
                  : [...filters.states, s];
                setFilter("states", next);
              }}
            >
              <Checkbox checked={filters.states.includes(s)} />
              <Label className="text-xs cursor-pointer">{s}</Label>
            </div>
          ))}
        </PopoverContent>
      </Popover>

      <Input
        type="text"
        placeholder={t("network.toolbar.search-placeholder")}
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        className="flex-1 max-w-xs"
      />
      {searchInput && (
        <Button
          variant="ghost"
          size="icon"
          onClick={() => setSearchInput("")}
          title="clear"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      )}

      <Select value={retentionValue} onValueChange={handleRetentionChange}>
        <SelectTrigger className="h-7 w-32 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="s60">{t("network.toolbar.retention-1m")}</SelectItem>
          <SelectItem value="s300">{t("network.toolbar.retention-5m")}</SelectItem>
          <SelectItem value="s600">{t("network.toolbar.retention-10m")}</SelectItem>
          <SelectItem value="forever">{t("network.toolbar.retention-forever")}</SelectItem>
        </SelectContent>
      </Select>

      <div className="flex-1" />

      <Button
        variant="destructive"
        size="sm"
        onClick={onKillSelected}
        disabled={!hasSelection}
      >
        <X className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.kill-process")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onExport} className="hover:shadow-sm transition-shadow">
        <Download className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.export-csv")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onClearHistory} className="hover:shadow-sm transition-shadow">
        <Trash2 className="h-3.5 w-3.5 mr-1" />
        {t("network.toolbar.clear-history")}
      </Button>
      <Button variant="ghost" size="icon" disabled={loading}>
        <RefreshCcw
          className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
        />
      </Button>
    </div>
  );
}
