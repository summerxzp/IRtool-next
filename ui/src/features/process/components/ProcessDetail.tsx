import { useState, useCallback, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { X, AlertTriangle } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { useProcessChain } from "../hooks";
import { iconCache, subscribePath } from "../columns";
import type { ProcessEntry, ProcessNode } from "../types";

interface Props {
  entry: ProcessEntry | null;
  snapshotTime: string | null;
  onClose?: () => void;
}

function ChainNodeIcon({ imagePath }: { imagePath: string | null }) {
  const subscribe = useCallback(
    (listener: () => void) => {
      if (!imagePath) return () => {};
      return subscribePath(imagePath, listener);
    },
    [imagePath]
  );

  const iconSrc = useSyncExternalStore(
    subscribe,
    () => {
      if (!imagePath) return null;
      const cached = iconCache.get(imagePath);
      return cached && cached !== "" ? cached : null;
    },
    () => null
  );

  if (iconSrc) {
    return <img src={iconSrc} alt="" className="w-4 h-4 shrink-0" />;
  }
  return <span className="w-4 h-4 shrink-0 inline-block rounded-sm bg-bg-elev-2" />;
}

function ChainNodeDetail({ node }: { node: ProcessNode }) {
  const { t } = useTranslation();
  return (
    <div className="ml-2 pl-2 border-l-2 border-border space-y-1 py-1">
      {node.exe && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.path")}: </span>
          <span className="font-mono break-all">{node.exe}</span>
        </div>
      )}
      {node.cmdline && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.cmdline")}: </span>
          <span className="font-mono break-all">{node.cmdline}</span>
        </div>
      )}
      {node.create_time && (
        <div className="text-xs">
          <span className="text-fg-tertiary">{t("process.detail.create-time")}: </span>
          <span>{node.create_time}</span>
        </div>
      )}
      {node.is_suspicious && node.suspicious_reason && (
        <div className="text-xs text-warning flex items-center gap-0.5">
          <AlertTriangle className="h-3 w-3" />
          {node.suspicious_reason}
        </div>
      )}
    </div>
  );
}

export function ProcessDetail({ entry, snapshotTime, onClose }: Props) {
  const { t } = useTranslation();
  const chainQuery = useProcessChain(entry?.pid ?? null);
  const [expandedIdx, setExpandedIdx] = useState<number | null>(null);

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
          <div className="space-y-0.5 pl-2">
            {[...chain.nodes].reverse().map((node, i) => {
              const originalIdx = chain.nodes.length - 1 - i;
              const isExpanded = expandedIdx === originalIdx;
              return (
                <div key={node.pid}>
                  <div
                    className={`flex items-center gap-1 text-xs cursor-pointer rounded px-1 py-0.5 -mx-1 hover:bg-bg-secondary ${node.is_target ? "bg-accent" : ""}`}
                    style={{ paddingLeft: `${i * 12 + 4}px` }}
                    onClick={() => setExpandedIdx(isExpanded ? null : originalIdx)}
                  >
                    <span className="text-fg-tertiary">└─</span>
                    <ChainNodeIcon imagePath={node.exe} />
                    <span className={node.is_target ? "text-fg-primary font-medium" : "text-fg-secondary"}>
                      {node.name} ({node.pid})
                    </span>
                    {node.is_suspicious && (
                      <span className="text-[10px] text-warning">⚠ {node.suspicious_reason}</span>
                    )}
                  </div>
                  {isExpanded && <ChainNodeDetail node={chain.nodes[originalIdx]} />}
                </div>
              );
            })}
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
