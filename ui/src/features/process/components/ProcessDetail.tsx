import { useTranslation } from "react-i18next";
import { X, AlertTriangle } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { useProcessChain } from "../hooks";
import type { ProcessEntry } from "../types";

interface Props {
  entry: ProcessEntry | null;
  snapshotTime: string | null;
  onClose?: () => void;
}

export function ProcessDetail({ entry, snapshotTime, onClose }: Props) {
  const { t } = useTranslation();
  const chainQuery = useProcessChain(entry?.pid ?? null);

  if (!entry) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("process.detail.select-row")}
      </div>
    );
  }

  const chain = chainQuery.data;
  const isLoading = chainQuery.isLoading;

  return (
    <div className="h-full overflow-auto p-4 space-y-3">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="text-sm font-medium text-fg-primary">{entry.name}</div>
          <div className="flex items-center gap-1.5 mt-1 text-xs text-fg-tertiary flex-wrap">
            <span className="font-mono">PID {entry.pid}</span>
            <span>·</span>
            <span className="font-mono">PPID {entry.ppid}</span>
            {entry.is_suspicious && (
              <>
                <span>·</span>
                <span className="text-warning flex items-center gap-0.5">
                  <AlertTriangle className="h-3 w-3" />{entry.suspicious_reason}
                </span>
              </>
            )}
          </div>
        </div>
        {onClose && (
          <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>

      {entry.exe && (
        <div className="text-xs">
          <div className="text-fg-tertiary">{t("process.detail.path")}</div>
          <div className="font-mono break-all">{entry.exe}</div>
        </div>
      )}

      <Separator />

      <div className="space-y-1">
        <div className="text-xs font-medium text-fg-secondary select-none">{t("process.chain.title")}</div>
        {isLoading ? (
          <div className="text-xs text-fg-tertiary">{t("common.loading")}</div>
        ) : chain && chain.nodes.length > 0 ? (
          <div className="space-y-1">
            {chain.nodes.map((node, i) => (
              <div key={node.pid} className="flex items-start gap-1.5 text-xs">
                <span className="text-fg-tertiary shrink-0">{i > 0 && "→ "}</span>
                <div className={`min-w-0 ${node.is_suspicious ? "text-warning" : ""}`}>
                  <div className="flex items-center gap-1">
                    <span className={`font-mono text-fg-tertiary shrink-0`}>{node.pid}</span>
                    <span className={node.is_target ? "font-medium" : ""}>{node.name}</span>
                    {node.is_suspicious && <span className="text-warning" title={node.suspicious_reason ?? undefined}>⚠</span>}
                  </div>
                  {node.exe && <div className="font-mono text-fg-tertiary truncate">{node.exe}</div>}
                  {node.cmdline && <div className="font-mono text-fg-tertiary truncate">{node.cmdline}</div>}
                  {node.create_time && <div className="text-fg-tertiary">{node.create_time}</div>}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-xs text-fg-tertiary">{t("process.chain.empty")}</div>
        )}
      </div>

      {snapshotTime && (
        <>
          <Separator />
          <div className="text-xs text-fg-tertiary select-none">
            {t("process.detail.snapshot-time")}: {snapshotTime}
          </div>
        </>
      )}
    </div>
  );
}
