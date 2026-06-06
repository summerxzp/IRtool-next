import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { EventConfigEntry } from "../types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  eventConfigs: EventConfigEntry[];
  onApply: (enabledEvents: string[], logSizeMb: number) => void;
  loading: boolean;
  currentLogSizeMb: number;
  /** Currently enabled event keys from the active config. If empty, falls back to defaults. */
  currentEnabledKeys?: string[];
}

export function SysmonConfigDialog({ open, onOpenChange, eventConfigs, onApply, loading, currentLogSizeMb, currentEnabledKeys }: Props) {
  const { t } = useTranslation();
  const [enabledKeys, setEnabledKeys] = useState<Set<string>>(new Set());
  const [logSizeMb, setLogSizeMb] = useState(64);

  useEffect(() => {
    if (open) {
      // Use current enabled keys if available, otherwise fall back to defaults
      const keys = (currentEnabledKeys && currentEnabledKeys.length > 0)
        ? currentEnabledKeys
        : eventConfigs.filter((c) => c.default_enabled).map((c) => c.key);
      setEnabledKeys(new Set(keys));
      setLogSizeMb(currentLogSizeMb > 0 ? currentLogSizeMb : 64);
    }
  }, [open, eventConfigs, currentLogSizeMb, currentEnabledKeys]);

  const toggleKey = (key: string) => {
    setEnabledKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleApply = () => {
    onApply(Array.from(enabledKeys), logSizeMb);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("log-collector.config-dialog.title")}</DialogTitle>
          <DialogDescription>{t("log-collector.config-dialog.description")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <div className="flex items-center justify-between">
            <p className="text-xs font-medium text-fg-secondary">{t("log-collector.install.event-config")}</p>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" className="h-5 text-[10px] px-1.5" onClick={() => setEnabledKeys(new Set(eventConfigs.map((c) => c.key)))}>
                {t("log-collector.config-dialog.select-all")}
              </Button>
              <Button variant="ghost" size="sm" className="h-5 text-[10px] px-1.5" onClick={() => setEnabledKeys(new Set())}>
                {t("log-collector.config-dialog.deselect-all")}
              </Button>
            </div>
          </div>
          <TooltipProvider delayDuration={300}>
            <div className="space-y-1.5 max-h-48 overflow-y-auto">
              {eventConfigs.map((cfg) => (
                <Tooltip key={cfg.key}>
                  <TooltipTrigger asChild>
                    <div className="flex items-center gap-2 px-2 py-1 rounded hover:bg-bg-elev-2 cursor-pointer" onClick={() => toggleKey(cfg.key)}>
                      <Checkbox
                        id={`cfg-evt-${cfg.key}`}
                        checked={enabledKeys.has(cfg.key)}
                        onCheckedChange={() => toggleKey(cfg.key)}
                      />
                      <Label htmlFor={`cfg-evt-${cfg.key}`} className="text-xs cursor-pointer flex-1">
                        {cfg.name}
                      </Label>
                      <span className="text-[10px] text-fg-tertiary">ID {cfg.event_id}</span>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent side="left" className="text-xs max-w-xs">
                    {t(`log-collector.install.event-desc.${cfg.key}`, `${cfg.xml_tag} (EventID ${cfg.event_id})`)}
                  </TooltipContent>
                </Tooltip>
              ))}
            </div>
          </TooltipProvider>

          <div className="flex items-center gap-2 pt-2 border-t border-border">
            <Label className="text-xs shrink-0">{t("log-collector.config-dialog.log-size")}</Label>
            <Input
              type="number"
              min={1}
              max={4096}
              value={logSizeMb}
              onChange={(e) => {
                const v = parseInt(e.target.value);
                if (!isNaN(v) && v > 0) setLogSizeMb(v);
              }}
              className="h-7 w-20 text-xs"
            />
            <span className="text-xs text-fg-tertiary">{t("log-collector.config-dialog.log-size-unit")}</span>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)} disabled={loading}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" onClick={handleApply} disabled={loading || enabledKeys.size === 0}>
            {loading ? t("log-collector.config-dialog.applying") : t("log-collector.config-dialog.apply")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
