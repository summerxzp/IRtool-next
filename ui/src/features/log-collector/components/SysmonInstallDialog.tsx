import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { EventConfigEntry } from "../types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  eventConfigs: EventConfigEntry[];
  onInstall: (acceptEula: boolean, enabledEvents: string[]) => void;
  loading: boolean;
}

export function SysmonInstallDialog({ open, onOpenChange, eventConfigs, onInstall, loading }: Props) {
  const { t } = useTranslation();
  const [enabledKeys, setEnabledKeys] = useState<Set<string>>(() => {
    const defaults = eventConfigs.filter((c) => c.default_enabled).map((c) => c.key);
    return new Set(defaults);
  });

  const toggleKey = (key: string) => {
    setEnabledKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleInstall = () => {
    onInstall(true, Array.from(enabledKeys));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("log-collector.install.title")}</DialogTitle>
          <DialogDescription>{t("log-collector.install.description")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          <p className="text-xs font-medium text-fg-secondary">{t("log-collector.install.event-config")}</p>
          <TooltipProvider delayDuration={300}>
            <div className="space-y-1.5 max-h-60 overflow-y-auto">
              {eventConfigs.map((cfg) => (
                <Tooltip key={cfg.key}>
                  <TooltipTrigger asChild>
                    <div className="flex items-center gap-2 px-2 py-1 rounded hover:bg-bg-elev-2 cursor-pointer" onClick={() => toggleKey(cfg.key)}>
                      <Checkbox
                        id={`evt-${cfg.key}`}
                        checked={enabledKeys.has(cfg.key)}
                        onCheckedChange={() => toggleKey(cfg.key)}
                      />
                      <Label htmlFor={`evt-${cfg.key}`} className="text-xs cursor-pointer flex-1">
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

          <div className="flex items-start gap-2 mt-3 p-2 bg-yellow-500/10 rounded text-xs text-yellow-500">
            <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
            <div>
              <p>{t("log-collector.install.warning-admin")}</p>
              <p className="mt-1 text-yellow-500/70">{t("log-collector.install.warning-driver")}</p>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)} disabled={loading}>
            {t("common.cancel")}
          </Button>
          <Button size="sm" onClick={handleInstall} disabled={loading || enabledKeys.size === 0}>
            {loading ? t("log-collector.install.installing") : t("log-collector.install.accept-and-install")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
