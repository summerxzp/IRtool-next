import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Plus, X } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { Rule, RuleTarget, Severity, Condition, ConditionType } from "../types";
import { getFieldsForTarget } from "../types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  rule: Rule | null;
  onSave: (rule: Rule) => void;
}

const TARGETS: RuleTarget[] = ["Autorun", "Network", "Event"];
const SEVERITIES: Severity[] = ["critical", "high", "medium", "low"];
const CONDITION_TYPES: ConditionType[] = ["contains", "regex", "equals"];

/** IOC fields that support multi-value input */
const IOC_FIELDS = new Set(["remote.addr", "destination_ip", "query_name"]);

function emptyCondition(target: RuleTarget): Condition {
  const fields = getFieldsForTarget(target);
  return { field: fields[0]?.key ?? "", type: "contains", value: "" };
}

function emptyRule(): Rule {
  return {
    id: `rule-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    name: "",
    target: "Autorun",
    conditions: [emptyCondition("Autorun")],
    logic: "and",
    severity: "medium",
    family: "",
    enabled: true,
  };
}

export function RuleEditDialog({ open, onOpenChange, rule, onSave }: Props) {
  const { t } = useTranslation();
  const isEdit = rule != null;

  const [form, setForm] = useState<Rule>(rule ?? emptyRule());

  // Reset form when rule or dialog opens
  useEffect(() => {
    if (open) {
      setForm(rule ?? emptyRule());
    }
  }, [open, rule]);

  const fields = getFieldsForTarget(form.target);
  const isIocField = (field: string) => IOC_FIELDS.has(field);

  const handleAddCondition = () => {
    setForm((prev) => ({
      ...prev,
      conditions: [...prev.conditions, emptyCondition(prev.target)],
    }));
  };

  const handleRemoveCondition = (index: number) => {
    setForm((prev) => ({
      ...prev,
      conditions: prev.conditions.filter((_, i) => i !== index),
    }));
  };

  const handleConditionChange = (index: number, partial: Partial<Condition>) => {
    setForm((prev) => ({
      ...prev,
      conditions: prev.conditions.map((c, i) => (i === index ? { ...c, ...partial } : c)),
    }));
  };

  const handleTargetChange = (newTarget: RuleTarget) => {
    setForm((prev) => ({
      ...prev,
      target: newTarget,
      conditions: [emptyCondition(newTarget)],
    }));
  };

  const buildRule = (): Rule => ({
    ...form,
    name: form.name.trim(),
    family: form.family.trim(),
    conditions: form.conditions.filter((c) => c.value.trim()),
  });

  const isValid = form.name.trim() && form.conditions.some((c) => c.value.trim());

  const handleApply = () => {
    if (!isValid) return;
    onSave(buildRule());
  };

  const handleCancel = () => {
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{isEdit ? t("workspace.rules.edit-rule") : t("workspace.rules.add-rule")}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3">
          <div>
            <Label className="text-xs">{t("workspace.rules.rule-name")}</Label>
            <Input value={form.name} onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))} className="mt-1" />
          </div>

          <div className="grid grid-cols-3 gap-2">
            <div>
              <Label className="text-xs">{t("workspace.rules.target")}</Label>
              <Select value={form.target} onValueChange={(v) => handleTargetChange(v as RuleTarget)}>
                <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {TARGETS.map((t) => <SelectItem key={t} value={t}>{t}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label className="text-xs">{t("workspace.rules.severity")}</Label>
              <Select value={form.severity} onValueChange={(v) => setForm((p) => ({ ...p, severity: v as Severity }))}>
                <SelectTrigger className="mt-1"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {SEVERITIES.map((s) => <SelectItem key={s} value={s}>{s}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label className="text-xs">{t("workspace.rules.family")}</Label>
              <Input value={form.family} onChange={(e) => setForm((p) => ({ ...p, family: e.target.value }))} className="mt-1" />
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2">
                <Label className="text-xs">{t("workspace.rules.conditions")}</Label>
                <div className="flex items-center gap-1 text-xs">
                  <Button
                    variant={form.logic === "and" ? "default" : "secondary"}
                    size="sm"
                    className="h-5 px-2 text-xs"
                    onClick={() => setForm((p) => ({ ...p, logic: "and" }))}
                  >
                    AND
                  </Button>
                  <Button
                    variant={form.logic === "or" ? "default" : "secondary"}
                    size="sm"
                    className="h-5 px-2 text-xs"
                    onClick={() => setForm((p) => ({ ...p, logic: "or" }))}
                  >
                    OR
                  </Button>
                </div>
              </div>
              <Button variant="ghost" size="sm" onClick={handleAddCondition}>
                <Plus className="h-3.5 w-3.5 mr-1" />
                {t("workspace.rules.add-condition")}
              </Button>
            </div>
            <div className="space-y-2">
              {form.conditions.map((cond, i) => {
                const isIoc = isIocField(cond.field);
                return (
                  <div key={i} className="space-y-1">
                    <div className="flex items-center gap-1.5">
                      <Select value={cond.field} onValueChange={(v) => handleConditionChange(i, { field: v, value: "" })}>
                        <SelectTrigger className="w-28 h-7 text-xs">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {fields.map((f) => (
                            <SelectItem key={f.key} value={f.key}>{f.label}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      <Select value={cond.type} onValueChange={(v) => handleConditionChange(i, { type: v as ConditionType })}>
                        <SelectTrigger className="w-24 h-7 text-xs">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          {CONDITION_TYPES.map((ct) => (
                            <SelectItem key={ct} value={ct}>{ct}</SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      <div className="flex-1" />
                      <Button variant="ghost" size="icon" className="shrink-0 h-7 w-7" onClick={() => handleRemoveCondition(i)} disabled={form.conditions.length <= 1}>
                        <X className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                    {isIoc ? (
                      <textarea
                        value={cond.value}
                        onChange={(e) => handleConditionChange(i, { value: e.target.value })}
                        onKeyDown={(e) => { if (e.key === "Enter") e.stopPropagation(); }}
                        placeholder={t("workspace.rules.ioc-placeholder")}
                        className="w-full h-20 rounded-md border border-border bg-transparent px-2 py-1 text-xs font-mono resize-y focus:outline-none focus:ring-1 focus:ring-accent"
                      />
                    ) : (
                      <Input
                        value={cond.value}
                        onChange={(e) => handleConditionChange(i, { value: e.target.value })}
                        placeholder={t("workspace.rules.value-placeholder")}
                        className="h-7 text-xs"
                      />
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={handleCancel}>
            {t("common.cancel")}
          </Button>
          <Button onClick={handleApply} disabled={!isValid}>
            {t("workspace.rules.apply")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
