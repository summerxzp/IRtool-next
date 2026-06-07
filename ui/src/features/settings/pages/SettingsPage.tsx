import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2, Eye, EyeOff, Send } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

type NotifyAction = "popup" | { feishu: { webhook_url: string } };

interface MonitorRule {
  name: string;
  targets: string[];
  event_types: string[];
  actions: NotifyAction[];
  enabled: boolean;
}

interface MonitorConfig {
  background_mode: boolean;
  persist_event_types: string[];
  retention_days: number;
  rules: MonitorRule[];
  db_path: string;
  enable_sni: boolean;
  enable_dns_pcap: boolean;
  load_limit: number;
}

const EVENT_TYPE_OPTIONS = [
  { key: "dns", label: "DNS查询" },
  { key: "dns_client", label: "DNS-Client" },
  { key: "network_connect", label: "网络连接" },
  { key: "tls_sni", label: "TLS SNI" },
  { key: "dns_pcap", label: "DNS抓包" },
  { key: "create_remote_thread", label: "远程线程" },
  { key: "file_create", label: "文件创建" },
];

function createEmptyRule(): MonitorRule {
  return {
    name: "",
    targets: [],
    event_types: [],
    actions: [],
    enabled: true,
  };
}

export default function SettingsPage() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<MonitorConfig>({
    background_mode: false,
    persist_event_types: [],
    retention_days: 7,
    rules: [],
    db_path: "",
    enable_sni: true,
    enable_dns_pcap: true,
    load_limit: 1000,
  });
  const [loading, setLoading] = useState(false);
  const [targetsInput, setTargetsInput] = useState<Record<number, string>>({});
  const [feishuUrls, setFeishuUrls] = useState<Record<number, string>>({});
  const [revealedUrls, setRevealedUrls] = useState<Record<number, boolean>>({});
  const [testingFeishu, setTestingFeishu] = useState<Record<number, boolean>>({});

  useEffect(() => {
    invoke<MonitorConfig>("cmd_monitor_get_config").then((c) => {
      setConfig(c);
      const ti: Record<number, string> = {};
      const fu: Record<number, string> = {};
      c.rules.forEach((r, i) => {
        ti[i] = r.targets.join(", ");
        const feishuAction = r.actions.find((a): a is { feishu: { webhook_url: string } } => typeof a === "object" && "feishu" in a);
        fu[i] = feishuAction?.feishu?.webhook_url ?? "";
      });
      setTargetsInput(ti);
      setFeishuUrls(fu);
    }).catch(() => {});
  }, []);

  const addRule = () => {
    setConfig((prev) => ({ ...prev, rules: [...prev.rules, createEmptyRule()] }));
  };

  const removeRule = (index: number) => {
    setConfig((prev) => ({ ...prev, rules: prev.rules.filter((_, i) => i !== index) }));
    setTargetsInput((prev) => {
      const next: Record<number, string> = {};
      Object.keys(prev).forEach((k) => {
        const ki = parseInt(k);
        if (ki < index) next[ki] = prev[ki];
        else if (ki > index) next[ki - 1] = prev[ki];
      });
      return next;
    });
    setFeishuUrls((prev) => {
      const next: Record<number, string> = {};
      Object.keys(prev).forEach((k) => {
        const ki = parseInt(k);
        if (ki < index) next[ki] = prev[ki];
        else if (ki > index) next[ki - 1] = prev[ki];
      });
      return next;
    });
  };

  const updateRule = (index: number, updates: Partial<MonitorRule>) => {
    setConfig((prev) => ({
      ...prev,
      rules: prev.rules.map((r, i) => (i === index ? { ...r, ...updates } : r)),
    }));
  };

  const toggleEventType = (ruleIndex: number, eventType: string) => {
    const rule = config.rules[ruleIndex];
    const types = rule.event_types.includes(eventType)
      ? rule.event_types.filter((t) => t !== eventType)
      : [...rule.event_types, eventType];
    updateRule(ruleIndex, { event_types: types });
  };

  const hasPopup = (rule: MonitorRule) => rule.actions.some((a) => a === "popup");
  const hasFeishu = (rule: MonitorRule) => rule.actions.some((a) => typeof a === "object" && "feishu" in a);

  const toggleAction = (ruleIndex: number, actionType: "popup" | "feishu") => {
    const rule = config.rules[ruleIndex];
    let newActions: NotifyAction[];
    if (actionType === "popup") {
      newActions = hasPopup(rule)
        ? rule.actions.filter((a) => a !== "popup")
        : [...rule.actions, "popup"];
    } else {
      newActions = hasFeishu(rule)
        ? rule.actions.filter((a) => !(typeof a === "object" && "feishu" in a))
        : [...rule.actions, { feishu: { webhook_url: feishuUrls[ruleIndex] ?? "" } }];
    }
    updateRule(ruleIndex, { actions: newActions });
  };

  const maskUrl = (url: string) => {
    if (!url) return "";
    try {
      const u = new URL(url);
      const path = u.pathname;
      if (path.length > 8) {
        u.pathname = path.slice(0, 4) + "****" + path.slice(-4);
      }
      return u.toString();
    } catch {
      return url.slice(0, 8) + "****";
    }
  };

  const testFeishu = async (ruleIndex: number) => {
    const url = feishuUrls[ruleIndex];
    if (!url) {
      toast.error(t("log-collector.monitor.webhook-url-required"));
      return;
    }
    setTestingFeishu((prev) => ({ ...prev, [ruleIndex]: true }));
    try {
      await invoke("cmd_monitor_test_feishu", { webhookUrl: url });
      toast.success(t("log-collector.monitor.test-feishu-success"));
    } catch (e) {
      toast.error(t("log-collector.monitor.test-feishu-failed"), { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setTestingFeishu((prev) => ({ ...prev, [ruleIndex]: false }));
    }
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      const updatedRules = config.rules.map((r, i) => ({
        ...r,
        targets: (targetsInput[i] ?? "").split(",").map((s) => s.trim()).filter(Boolean),
        actions: r.actions.map((a) => {
          if (typeof a === "object" && "feishu" in a) {
            return { feishu: { webhook_url: feishuUrls[i] ?? a.feishu.webhook_url } };
          }
          return a;
        }),
      }));
      await invoke("cmd_monitor_update_config", { config: { ...config, rules: updatedRules } });
      toast.success(t("settings.save-success"));
    } catch (e) {
      toast.error(t("settings.save-failed"), { description: e instanceof Error ? e.message : "" });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex h-full">
      {/* Left tab navigation */}
      <div className="w-40 border-r border-border bg-bg-elev-1 py-3 px-2">
        <p className="text-xs font-medium text-fg-secondary px-2 mb-2">{t("settings.title")}</p>
        <button
          className="w-full text-left text-xs px-2 py-1.5 rounded-md bg-accent/10 text-accent"
        >
          {t("settings.tabs.notification")}
        </button>
      </div>

      {/* Right content area */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="max-w-2xl space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium">{t("settings.notification.title")}</h2>
            <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={addRule}>
              <Plus className="h-3 w-3 mr-1" />
              {t("log-collector.monitor.add-rule")}
            </Button>
          </div>

          {config.rules.length === 0 && (
            <p className="text-xs text-fg-tertiary py-4 text-center">{t("log-collector.monitor.no-alerts")}</p>
          )}

          <div className="space-y-3">
            {config.rules.map((rule, i) => (
              <div key={i} className="border border-border rounded-md p-3 space-y-2">
                <div className="flex items-center gap-2">
                  <Input
                    placeholder={t("log-collector.monitor.rule-name")}
                    value={rule.name}
                    onChange={(e) => updateRule(i, { name: e.target.value })}
                    className="h-7 text-xs flex-1"
                  />
                  <Checkbox checked={rule.enabled} onCheckedChange={(v) => updateRule(i, { enabled: v === true })} />
                  <Label className="text-[10px]">{rule.enabled ? t("log-collector.monitor.enabled") : t("log-collector.monitor.disabled")}</Label>
                  <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => removeRule(i)}>
                    <Trash2 className="h-3 w-3 text-red-500" />
                  </Button>
                </div>

                <Input
                  placeholder={t("log-collector.monitor.target-placeholder")}
                  value={targetsInput[i] ?? ""}
                  onChange={(e) => setTargetsInput((prev) => ({ ...prev, [i]: e.target.value }))}
                  className="h-7 text-xs"
                />

                <div className="flex flex-wrap gap-1.5">
                  {EVENT_TYPE_OPTIONS.map((et) => (
                    <Badge
                      key={et.key}
                      variant={rule.event_types.includes(et.key) ? "default" : "outline"}
                      className="text-[10px] cursor-pointer"
                      onClick={() => toggleEventType(i, et.key)}
                    >
                      {et.label}
                    </Badge>
                  ))}
                  {rule.event_types.length === 0 && (
                    <span className="text-[10px] text-fg-tertiary">{t("log-collector.monitor.all-types")}</span>
                  )}
                </div>

                <div className="flex items-center gap-3">
                  <div className="flex items-center gap-1">
                    <Checkbox checked={hasPopup(rule)} onCheckedChange={() => toggleAction(i, "popup")} />
                    <Label className="text-[10px]">{t("log-collector.monitor.popup")}</Label>
                  </div>
                  <div className="flex items-center gap-1">
                    <Checkbox checked={hasFeishu(rule)} onCheckedChange={() => toggleAction(i, "feishu")} />
                    <Label className="text-[10px]">{t("log-collector.monitor.feishu")}</Label>
                  </div>
                  {hasFeishu(rule) && (
                    <div className="flex items-center gap-1.5 flex-1">
                      <div className="relative flex-1">
                        <Input
                          placeholder={t("log-collector.monitor.webhook-url")}
                          value={revealedUrls[i] ? (feishuUrls[i] ?? "") : maskUrl(feishuUrls[i] ?? "")}
                          onChange={(e) => {
                            if (revealedUrls[i]) {
                              setFeishuUrls((prev) => ({ ...prev, [i]: e.target.value }));
                            }
                          }}
                          onFocus={() => setRevealedUrls((prev) => ({ ...prev, [i]: true }))}
                          className="h-7 text-xs pr-8"
                          readOnly={!revealedUrls[i]}
                        />
                        <button
                          type="button"
                          className="absolute right-1.5 top-1/2 -translate-y-1/2 text-fg-tertiary hover:text-fg-primary"
                          onClick={() => setRevealedUrls((prev) => ({ ...prev, [i]: !prev[i] }))}
                        >
                          {revealedUrls[i] ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                        </button>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 shrink-0"
                        onClick={() => testFeishu(i)}
                        disabled={testingFeishu[i] || !feishuUrls[i]}
                      >
                        <Send className="h-3 w-3" />
                      </Button>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>

          <div className="space-y-2 pt-3 border-t border-border">
            <div className="flex items-center gap-2">
              <Label className="text-xs shrink-0 w-24">{t("log-collector.monitor.retention-days")}</Label>
              <Input
                type="number"
                min={0}
                value={config.retention_days}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  if (!isNaN(v) && v >= 0) setConfig((prev) => ({ ...prev, retention_days: v }));
                }}
                className="h-7 w-20 text-xs"
              />
              <span className="text-xs text-fg-tertiary">0 = {t("log-collector.monitor.all-types").toLowerCase()}</span>
            </div>
            <div className="flex items-center gap-2">
              <Label className="text-xs shrink-0 w-24">加载条数</Label>
              <Input
                type="number"
                min={100}
                max={100000}
                value={config.load_limit}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  if (!isNaN(v) && v >= 100 && v <= 100000) setConfig((prev) => ({ ...prev, load_limit: v }));
                }}
                className="h-7 w-20 text-xs"
              />
              <span className="text-xs text-fg-tertiary">每次从数据库加载的事件数量</span>
            </div>
            <div className="flex items-center gap-2">
              <Label className="text-xs shrink-0 w-24">{t("log-collector.monitor.db-path")}</Label>
              <Input
                type="text"
                placeholder={t("log-collector.monitor.db-path-placeholder")}
                value={config.db_path}
                onChange={(e) => setConfig((prev) => ({ ...prev, db_path: e.target.value }))}
                className="h-7 text-xs flex-1"
              />
            </div>
          </div>

          <div className="flex justify-end pt-2">
            <Button size="sm" onClick={handleSave} disabled={loading}>
              {loading ? t("log-collector.monitor.saving") : t("log-collector.monitor.save")}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
