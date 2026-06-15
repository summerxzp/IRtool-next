import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Copy, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { commands } from "@/lib/bindings";
import { formatEventTimestamp } from "@/lib/utils";
import { EVENT_TYPE_LABELS } from "../types";
import type { SysmonEvent, ExtendedSysmonEventType } from "../types";

interface Props {
  event: SysmonEvent | null;
  onClose?: () => void;
}

function FieldRow({ label, value, copyable }: { label: string; value: string; copyable?: boolean }) {
  if (!value) return null;
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="text-fg-tertiary w-20 shrink-0 text-right">{label}</span>
      <span className="text-fg-primary break-all flex-1">{value}</span>
      {copyable && (
        <Button variant="ghost" size="icon" className="h-5 w-5 shrink-0" onClick={() => navigator.clipboard.writeText(value)}>
          <Copy className="h-3 w-3" />
        </Button>
      )}
    </div>
  );
}

function ProcessChain({ pid }: { pid: number }) {
  const { t } = useTranslation();
  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);
  const { data: chain } = useQuery({
    queryKey: ["process-chain", pid],
    queryFn: async () => {
      const result = await commands.cmdProcessChain(pid);
      if (result.status === "error") throw result.error;
      return result.data;
    },
    enabled: pid > 0,
  });

  if (!chain || !chain.nodes || chain.nodes.length <= 1) return null;

  const reversed = [...chain.nodes].reverse();
  const selectedNode = selectedIdx !== null ? chain.nodes[selectedIdx] : null;

  return (
    <div className="mt-2">
      <p className="text-xs font-medium text-fg-secondary mb-1">{t("log-collector.detail.process-chain")}</p>
      <div className="space-y-0.5 pl-2">
        {reversed.map((node, i) => {
          const originalIdx = chain.nodes.length - 1 - i;
          const isSelected = selectedIdx === originalIdx;
          return (
            <div
              key={node.pid}
              className={`flex items-center gap-1 text-xs cursor-pointer rounded px-1 py-0.5 -mx-1 hover:bg-bg-secondary ${isSelected ? "bg-bg-secondary" : ""}`}
              style={{ paddingLeft: `${i * 12 + 4}px` }}
              onClick={() => setSelectedIdx(isSelected ? null : originalIdx)}
            >
              <span className="text-fg-tertiary">└─</span>
              <span className={node.is_target ? "text-fg-primary font-medium" : "text-fg-secondary"}>
                {node.name} ({node.pid})
              </span>
              {node.is_suspicious && (
                <span className="text-[10px] text-yellow-500">⚠ {node.suspicious_reason}</span>
              )}
            </div>
          );
        })}
      </div>
      {selectedNode && (
        <div className="mt-2 ml-2 pl-2 border-l-2 border-border space-y-1">
          <FieldRow label={t("log-collector.detail.process")} value={`${selectedNode.name} (${selectedNode.pid})`} />
          <FieldRow label={t("log-collector.detail.path")} value={selectedNode.exe ?? ""} copyable />
          {selectedNode.cmdline && (
            <FieldRow label={t("log-collector.detail.command-line")} value={selectedNode.cmdline} copyable />
          )}
          {selectedNode.create_time && (
            <FieldRow label={t("log-collector.detail.create-time")} value={selectedNode.create_time} />
          )}
          {selectedNode.is_suspicious && selectedNode.suspicious_reason && (
            <FieldRow label={t("log-collector.detail.suspicious")} value={selectedNode.suspicious_reason} />
          )}
        </div>
      )}
    </div>
  );
}

function formatDisplayTimestamp(ts: string | number): string {
  if (typeof ts === 'number') {
    return formatEventTimestamp(ts);
  }
  return ts;
}

export function EventDetail({ event, onClose }: Props) {
  const { t } = useTranslation();

  if (!event) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-fg-tertiary">
        {t("log-collector.detail.select-event")}
      </div>
    );
  }

  return (
    <div className="p-3 space-y-2 overflow-auto h-full">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{EVENT_TYPE_LABELS[event.event_type as ExtendedSysmonEventType]}</span>
          <span className="text-[10px] text-fg-tertiary">EventID {event.event_id}</span>
        </div>
        {onClose && (
          <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
      <Separator />

      <FieldRow label={t("log-collector.detail.time")} value={formatDisplayTimestamp(event.timestamp)} />
      <FieldRow label={t("log-collector.detail.process")} value={event.process_name && event.process_name !== "<unknown process>" ? `${event.process_name} (${event.process_id})` : `PID: ${event.process_id}`} />
      <FieldRow label={t("log-collector.detail.path")} value={event.process_path === "<unknown process>" ? "" : event.process_path} copyable />
      <FieldRow label={t("log-collector.detail.user")} value={event.user} />

      {(event.event_type as ExtendedSysmonEventType) === "dns" || (event.event_type as ExtendedSysmonEventType) === "dns_client" ? (
        <>
          <Separator />
          <FieldRow label={t("log-collector.detail.domain")} value={event.query_name} copyable />
          <FieldRow label={t("log-collector.detail.results")} value={event.query_results} copyable />
          <FieldRow label={t("log-collector.detail.status")} value={event.query_status > 0 ? String(event.query_status) : "0 (Success)"} />
        </>
      ) : null}

      {((event.event_type as ExtendedSysmonEventType) === "tls_sni" || (event.event_type as ExtendedSysmonEventType) === "dns_pcap") && (
        <>
          <Separator />
          <FieldRow label={t("log-collector.detail.domain")} value={event.query_name} copyable />
          {event.query_results && (
            <FieldRow label={t("log-collector.detail.results")} value={event.query_results} copyable />
          )}
          <FieldRow label={t("log-collector.detail.source")} value={`${event.source_ip}:${event.source_port}`} />
          <FieldRow label={t("log-collector.detail.destination")} value={`${event.destination_ip}:${event.destination_port}`} copyable />
          <FieldRow label={t("log-collector.detail.protocol")} value={event.protocol} />
        </>
      )}

      {event.event_type === "network_connect" && (
        <>
          <Separator />
          <FieldRow label={t("log-collector.detail.source")} value={`${event.source_ip}:${event.source_port}`} />
          <FieldRow label={t("log-collector.detail.destination")} value={`${event.destination_ip}:${event.destination_port}`} copyable />
          <FieldRow label={t("log-collector.detail.protocol")} value={event.protocol} />
          <FieldRow label={t("log-collector.detail.initiated")} value={event.initiated ? "Yes" : "No"} />
          <FieldRow label={t("log-collector.detail.external")} value={event.is_external ? "Yes" : "No"} />
        </>
      )}

      {event.event_type === "create_remote_thread" && (
        <>
          <Separator />
          <FieldRow label={t("log-collector.detail.source-process")} value={`${event.source_process_name} (${event.source_process_id})`} />
          <FieldRow label={t("log-collector.detail.source-path")} value={event.source_process_path} copyable />
          <FieldRow label={t("log-collector.detail.target-process")} value={`${event.target_process_name} (${event.target_process_id})`} />
          <FieldRow label={t("log-collector.detail.target-path")} value={event.target_process_path} copyable />
          <FieldRow label={t("log-collector.detail.start-address")} value={event.start_address} copyable />
          <FieldRow label={t("log-collector.detail.start-module")} value={event.start_module} />
        </>
      )}

      {event.event_type === "file_create" && (
        <>
          <Separator />
          <FieldRow label={t("log-collector.detail.filename")} value={event.target_filename} copyable />
          <FieldRow label={t("log-collector.detail.creation-time")} value={event.creation_utc_time} />
        </>
      )}

      {event.process_id > 0 && (
        <>
          <Separator />
          <ProcessChain pid={event.process_id} />
        </>
      )}
    </div>
  );
}
