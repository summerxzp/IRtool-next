import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { AutorunItem, ScanProgress } from "../types";
import { useAutorunsStore } from "../store";

interface Props {
  data: AutorunItem[];
  scanning: boolean;
  scanProgress: ScanProgress | null;
  verifyingSignatures: boolean;
  signatureProgress: { current: number; total: number } | null;
}

const PHASE_LABELS: Record<string, string> = {
  running_autorunsc: "运行 autorunsc",
  parsing_csv: "解析 CSV",
  checking_files: "检查文件",
  evaluating_risk: "评估风险",
  verifying_signatures: "验证签名",
  complete: "完成",
};

export function AutorunsStatsBar({ data, scanning, scanProgress, verifyingSignatures, signatureProgress }: Props) {
  const { t } = useTranslation();
  const lastScanDuration = useAutorunsStore((s) => s.lastScanDuration);
  const stats = useMemo(() => {
    let total = 0, signed = 0, disabled = 0;
    for (const item of data) {
      total++;
      if (!item.enabled) disabled++;
      if (item.signature.kind === "valid") signed++;
    }
    return { total, signed, disabled };
  }, [data]);

  return (
    <div className="h-7 px-3 flex items-center gap-4 bg-bg-elev-1 border-t border-border text-xs text-fg-secondary">
      <span>{t("autoruns.stats.total")}: <span className="text-fg-primary font-medium">{stats.total}</span></span>
      <span className="text-success">{t("autoruns.stats.signed")}: <span className="font-medium">{stats.signed}/{stats.total}</span></span>
      {stats.disabled > 0 && <span className="text-fg-tertiary">{t("autoruns.stats.disabled")}: {stats.disabled}</span>}
      <div className="flex-1" />
      {scanning && scanProgress && scanProgress.phase !== "complete" && (
        <span className="text-accent animate-pulse">
          {PHASE_LABELS[scanProgress.phase] ?? scanProgress.phase}…
        </span>
      )}
      {verifyingSignatures && signatureProgress && (
        <span className="text-accent">{t("autoruns.stats.verifying")}: {signatureProgress.current}/{signatureProgress.total}</span>
      )}
      {!scanning && lastScanDuration != null && (
        <span className="text-fg-tertiary">耗时 {lastScanDuration.toFixed(1)}s</span>
      )}
    </div>
  );
}
