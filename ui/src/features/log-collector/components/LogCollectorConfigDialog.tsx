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
  onApply: (enabledEvents: string[], logSizeMb: number, pcapConfig: { enable_sni: boolean; enable_dns_pcap: boolean }) => void;
  loading: boolean;
  currentLogSizeMb: number;
  /** Currently enabled event keys from the active config. If empty, falls back to defaults. */
  currentEnabledKeys?: string[];
  currentPcapConfig?: { enable_sni: boolean; enable_dns_pcap: boolean };
  pcapAvailable?: boolean;
}

const EVENT_SECTIONS = [
  {
    key: "dns",
    eventKeys: ["dns"],
    extras: ["tls_sni", "dns_pcap"] as const,
  },
  {
    key: "process",
    eventKeys: ["process_create", "create_remote_thread", "process_terminate", "process_access", "process_tampering"],
  },
  {
    key: "network",
    eventKeys: ["network_connect"],
  },
  {
    key: "file",
    eventKeys: ["file_create", "file_create_time", "file_delete", "file_delete_detected", "file_create_stream_hash", "registry_event", "raw_access_read"],
  },
  {
    key: "driver",
    eventKeys: ["driver_load", "image_load"],
  },
  {
    key: "other",
    eventKeys: ["pipe_event", "wmi_event", "clipboard_change"],
  },
];

export function LogCollectorConfigDialog({ open, onOpenChange, eventConfigs, onApply, loading, currentLogSizeMb, currentEnabledKeys, currentPcapConfig, pcapAvailable = true }: Props) {
  const { t } = useTranslation();
  const [enabledKeys, setEnabledKeys] = useState<Set<string>>(new Set());
  const [logSizeMb, setLogSizeMb] = useState(64);
  const [enableSni, setEnableSni] = useState(true);
  const [enableDnsPcap, setEnableDnsPcap] = useState(true);

  useEffect(() => {
    if (open) {
      const keys = (currentEnabledKeys && currentEnabledKeys.length > 0)
        ? currentEnabledKeys
        : eventConfigs.filter((c) => c.default_enabled).map((c) => c.key);
      setEnabledKeys(new Set(keys));
      setLogSizeMb(currentLogSizeMb > 0 ? currentLogSizeMb : 64);
      setEnableSni(currentPcapConfig?.enable_sni ?? true);
      setEnableDnsPcap(currentPcapConfig?.enable_dns_pcap ?? true);
    }
  }, [open, eventConfigs, currentLogSizeMb, currentEnabledKeys, currentPcapConfig]);

  const toggleKey = (key: string) => {
    setEnabledKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleApply = () => {
    onApply(Array.from(enabledKeys), logSizeMb, { enable_sni: enableSni, enable_dns_pcap: enableDnsPcap });
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("log-collector.config-dialog.title")}</DialogTitle>
          <DialogDescription>{t("log-collector.config-dialog.description")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2 max-h-[60vh] overflow-y-auto">
          <TooltipProvider delayDuration={300}>
            {EVENT_SECTIONS.map((section) => (
              <div key={section.key}>
                {/* Section header with divider - centered */}
                <div className="flex items-center gap-2 mb-1.5">
                  <div className="flex-1 h-px bg-border" />
                  <p className="text-[10px] font-medium text-fg-tertiary uppercase tracking-wider shrink-0">
                    {t(`log-collector.config-dialog.section-${section.key}`)}
                  </p>
                  <div className="flex-1 h-px bg-border" />
                </div>

                {/* Event checkboxes for this section */}
                <div className="space-y-1">
                  {eventConfigs
                    .filter((cfg) => section.eventKeys.includes(cfg.key))
                    .map((cfg) => (
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
                        <TooltipContent side="left" className="text-xs max-w-sm">
                          {t(`log-collector.install.event-desc.${cfg.key}`, `${cfg.xml_tag} (EventID ${cfg.event_id})`)}
                        </TooltipContent>
                      </Tooltip>
                    ))
                  }

                  {/* Extra items (TLS SNI, DNS PCAP) for DNS section */}
                  {"extras" in section && section.extras?.map((extraKey) => (
                    <Tooltip key={extraKey}>
                      <TooltipTrigger asChild>
                        <div
                          className={`flex items-center gap-2 px-2 py-1 rounded hover:bg-bg-elev-2 cursor-pointer ${!pcapAvailable ? "opacity-50" : ""}`}
                          onClick={() => {
                            if (!pcapAvailable) return;
                            if (extraKey === "tls_sni") setEnableSni(!enableSni);
                            else setEnableDnsPcap(!enableDnsPcap);
                          }}
                        >
                          <Checkbox
                            id={`cfg-evt-${extraKey}`}
                            checked={extraKey === "tls_sni" ? enableSni : enableDnsPcap}
                            onCheckedChange={(v) => {
                              if (extraKey === "tls_sni") setEnableSni(v === true);
                              else setEnableDnsPcap(v === true);
                            }}
                            disabled={!pcapAvailable}
                          />
                          <Label htmlFor={`cfg-evt-${extraKey}`} className="text-xs cursor-pointer flex-1">
                            {t(`log-collector.config-dialog.${extraKey}`)}
                          </Label>
                        </div>
                      </TooltipTrigger>
                      <TooltipContent side="left" className="text-xs max-w-sm">
                        <p>{t(`log-collector.config-dialog.${extraKey}-desc`)}</p>
                        <p className="mt-0.5 text-fg-tertiary">{t(`log-collector.config-dialog.${extraKey}-source`)}</p>
                      </TooltipContent>
                    </Tooltip>
                  ))}
                </div>
              </div>
            ))}
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
