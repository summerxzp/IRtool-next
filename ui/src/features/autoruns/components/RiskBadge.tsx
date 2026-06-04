import { Badge } from "@/components/ui/badge";
import type { RiskLevel } from "../types";

const RISK_VARIANT: Record<RiskLevel, "default" | "success" | "warning" | "danger"> = {
  safe: "success",
  suspicious: "warning",
  high_risk: "danger",
};

const RISK_LABEL: Record<RiskLevel, string> = {
  safe: "正常",
  suspicious: "可疑",
  high_risk: "高风险",
};

interface Props {
  level: RiskLevel;
}

export function RiskBadge({ level }: Props) {
  return <Badge variant={RISK_VARIANT[level]}>{RISK_LABEL[level]}</Badge>;
}
