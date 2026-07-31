import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Download, Upload, Pencil, Trash2, Search } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { useWorkspaceStore } from "../store";
import { saveRules, exportRules, importRules, importIocRules } from "../rules/storage";
import type { Rule } from "../types";
import { RuleEditDialog } from "./RuleEditDialog";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function RuleManagerDialog({ open, onOpenChange }: Props) {
  const { t } = useTranslation();
  const rules = useWorkspaceStore((s) => s.rules);
  const setRules = useWorkspaceStore((s) => s.setRules);
  const [searchQuery, setSearchQuery] = useState("");
  const [editRule, setEditRule] = useState<Rule | null>(null);
  const [editOpen, setEditOpen] = useState(false);

  const filteredRules = searchQuery.trim()
    ? rules.filter((r) =>
        r.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        r.family.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : rules;

  const handleToggle = (rule: Rule) => {
    const updated = rules.map((r) =>
      r.id === rule.id ? { ...r, enabled: !r.enabled } : r
    );
    setRules(updated);
    saveRules(updated);
  };

  const handleDelete = (rule: Rule) => {
    const updated = rules.filter((r) => r.id !== rule.id);
    setRules(updated);
    saveRules(updated);
  };

  const handleAdd = () => {
    setEditRule(null);
    setEditOpen(true);
  };

  const handleEdit = (rule: Rule) => {
    setEditRule(rule);
    setEditOpen(true);
  };

  const handleSaveRule = (rule: Rule) => {
    let updated: Rule[];
    if (editRule) {
      updated = rules.map((r) => (r.id === rule.id ? rule : r));
    } else {
      updated = [...rules, rule];
    }
    setRules(updated);
    saveRules(updated);
    setEditOpen(false);
  };

  const handleExport = () => {
    const json = exportRules(rules);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `irtool-rules-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImport = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        let imported: Rule[];
        try {
          imported = importRules(text);
        } catch {
          imported = importIocRules(text);
        }
        const updated = [...rules, ...imported];
        setRules(updated);
        saveRules(updated);
      } catch {
        alert(t("workspace.rules.import-failed"));
      }
    };
    input.click();
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>{t("workspace.rules.title")}</DialogTitle>
          </DialogHeader>

          <div className="flex items-center gap-2 mb-2">
            <div className="relative flex-1">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-fg-tertiary" />
              <Input
                type="text"
                placeholder={t("workspace.rules.search-placeholder")}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-7"
              />
            </div>
            <Button variant="secondary" size="sm" onClick={handleAdd}>
              <Plus className="h-3.5 w-3.5 mr-1" />
              {t("workspace.rules.add")}
            </Button>
            <Button variant="secondary" size="sm" onClick={handleImport}>
              <Upload className="h-3.5 w-3.5 mr-1" />
              {t("workspace.rules.import")}
            </Button>
            <Button variant="secondary" size="sm" onClick={handleExport}>
              <Download className="h-3.5 w-3.5 mr-1" />
              {t("workspace.rules.export")}
            </Button>
          </div>

          <div className="flex-1 overflow-auto space-y-1 min-h-0">
            {filteredRules.length === 0 ? (
              <div className="text-center text-fg-tertiary text-sm py-8">
                {t("workspace.rules.no-rules")}
              </div>
            ) : (
              filteredRules.map((rule) => (
                <div
                  key={rule.id}
                  className={`flex items-center gap-2 px-3 py-2 rounded-md text-sm ${
                    rule.enabled ? "bg-bg-base" : "bg-bg-base opacity-60"
                  }`}
                >
                  <button
                    className={`w-3 h-3 rounded-sm border ${
                      rule.enabled ? "bg-accent border-accent" : "border-border"
                    }`}
                    onClick={() => handleToggle(rule)}
                  />
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-fg-primary truncate">{rule.name}</div>
                    <div className="text-xs text-fg-tertiary">
                      {rule.target} · {rule.family} · {rule.conditions.length} 条件
                    </div>
                  </div>
                  <Badge
                    variant={
                      rule.severity === "critical" || rule.severity === "high"
                        ? "danger"
                        : rule.severity === "medium"
                        ? "warning"
                        : "default"
                    }
                  >
                    {rule.severity}
                  </Badge>
                  <Button variant="ghost" size="icon" aria-label="编辑规则" onClick={() => handleEdit(rule)}>
                    <Pencil className="h-3.5 w-3.5" />
                  </Button>
                  <Button variant="ghost" size="icon" aria-label="删除规则" onClick={() => handleDelete(rule)}>
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              ))
            )}
          </div>
        </DialogContent>
      </Dialog>

      <RuleEditDialog
        open={editOpen}
        onOpenChange={setEditOpen}
        rule={editRule}
        onSave={handleSaveRule}
      />
    </>
  );
}
