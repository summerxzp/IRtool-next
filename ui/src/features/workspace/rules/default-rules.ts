import type { Rule } from "../types";

export const DEFAULT_RULES: Rule[] = [
  {
    id: "default-temp-persistence",
    name: "Temp 目录可疑持久化",
    target: "Autorun",
    conditions: [{ field: "image_path", type: "contains", value: "\\Temp\\" }],
    severity: "high",
    family: "持久化",
    enabled: true,
    description: "检测从 Temp 目录启动的持久化项，常见于恶意软件",
  },
  {
    id: "default-appdata-persistence",
    name: "AppData 可疑持久化",
    target: "Autorun",
    conditions: [{ field: "image_path", type: "contains", value: "\\AppData\\" }],
    severity: "medium",
    family: "持久化",
    enabled: true,
    description: "检测从 AppData 目录启动的持久化项，部分合法软件也使用此路径",
  },
  {
    id: "default-unsigned-persistence",
    name: "未签名持久化项",
    target: "Autorun",
    conditions: [{ field: "publisher", type: "equals", value: "" }],
    severity: "low",
    family: "签名",
    enabled: false,
    description: "检测没有发布者信息的持久化项，可能需要进一步调查",
  },
];
