import type { Rule, ConditionType, Severity, RuleTarget } from "../types";
import { DEFAULT_RULES } from "./default-rules";

const STORAGE_KEY = "irtool-workspace-rules";
const VERSION_KEY = "irtool-workspace-rules-version";
const CURRENT_VERSION = 2; // Bump when default rules change and should be merged

const VALID_CONDITION_TYPES: ConditionType[] = ["contains", "regex", "equals"];
const VALID_SEVERITIES: Severity[] = ["critical", "high", "medium", "low"];
const VALID_TARGETS: RuleTarget[] = ["Autorun", "Network", "Event"];

function isValidRule(obj: unknown): obj is Rule {
  if (typeof obj !== "object" || obj === null) return false;
  const r = obj as Record<string, unknown>;
  return (
    typeof r.id === "string" &&
    typeof r.name === "string" &&
    VALID_TARGETS.includes(r.target as RuleTarget) &&
    Array.isArray(r.conditions) &&
    (r.conditions as unknown[]).every((c) => {
      if (typeof c !== "object" || c === null) return false;
      const cond = c as Record<string, unknown>;
      return (
        typeof cond.field === "string" &&
        VALID_CONDITION_TYPES.includes(cond.type as ConditionType) &&
        typeof cond.value === "string"
      );
    }) &&
    VALID_SEVERITIES.includes(r.severity as Severity) &&
    typeof r.family === "string" &&
    typeof r.enabled === "boolean" &&
    (r.description === undefined || typeof r.description === "string")
  );
}

export function loadRules(): Rule[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [...DEFAULT_RULES];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [...DEFAULT_RULES];
    const valid = parsed.filter(isValidRule);
    if (valid.length === 0) return [...DEFAULT_RULES];

    // Merge new default rules if version bumped
    const savedVersion = parseInt(localStorage.getItem(VERSION_KEY) ?? "0", 10);
    if (savedVersion < CURRENT_VERSION) {
      const existingIds = new Set(valid.map((r) => r.id));
      const newDefaults = DEFAULT_RULES.filter((r) => !existingIds.has(r.id));
      if (newDefaults.length > 0) {
        valid.push(...newDefaults);
        saveRules(valid);
      }
      localStorage.setItem(VERSION_KEY, String(CURRENT_VERSION));
    }

    return valid;
  } catch {
    return [...DEFAULT_RULES];
  }
}

export function saveRules(rules: Rule[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(rules));
}

export function exportRules(rules: Rule[]): string {
  return JSON.stringify(rules, null, 2);
}

export function importRules(json: string): Rule[] {
  const parsed: unknown = JSON.parse(json);
  if (!Array.isArray(parsed)) throw new Error("导入的 JSON 不是数组");
  const valid = parsed.filter(isValidRule);
  if (valid.length === 0) throw new Error("没有有效的规则可导入");
  return valid;
}

/** Import from legacy IRtool IOC format.
 *  Expected shape: { rules: { [name: string]: { target, field, type, value, severity, family, enabled?, description? } } }
 */
export function importIocRules(json: string): Rule[] {
  const parsed: unknown = JSON.parse(json);
  if (typeof parsed !== "object" || parsed === null || !("rules" in parsed))
    throw new Error("不是有效的 IRtool IOC 格式");

  const rulesObj = (parsed as { rules: Record<string, unknown> }).rules;
  if (typeof rulesObj !== "object" || rulesObj === null)
    throw new Error("rules 字段无效");

  const result: Rule[] = [];
  for (const [name, val] of Object.entries(rulesObj)) {
    if (typeof val !== "object" || val === null) continue;
    const v = val as Record<string, unknown>;
    const rule: Rule = {
      id: `ioc-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name,
      target: (VALID_TARGETS.includes(v.target as RuleTarget)
        ? v.target
        : "Autorun") as RuleTarget,
      conditions: [
        {
          field: typeof v.field === "string" ? v.field : "image_path",
          type: (VALID_CONDITION_TYPES.includes(v.type as ConditionType)
            ? v.type
            : "contains") as ConditionType,
          value: typeof v.value === "string" ? v.value : "",
        },
      ],
      severity: (VALID_SEVERITIES.includes(v.severity as Severity)
        ? v.severity
        : "medium") as Severity,
      family: typeof v.family === "string" ? v.family : "导入",
      enabled: typeof v.enabled === "boolean" ? v.enabled : true,
      description:
        typeof v.description === "string" ? v.description : undefined,
    };
    result.push(rule);
  }
  if (result.length === 0) throw new Error("没有有效的 IOC 规则可导入");
  return result;
}
