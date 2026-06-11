import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Plus, Trash2, Eye, EyeOff, Send, Database, Bell, ShieldAlert, Info, Download, Upload } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

interface AlertRule {
  id: string;
  name: string;
  targets: string[];
  event_types: string[];
  enabled: boolean;
}

interface NotifyConfig {
  popup_rule_ids: string[];
  feishu_rule_ids: string[];
  feishu_webhook_url: string;
  popup_duration_secs: number;
}

interface MonitorConfig {
  background_mode: boolean;
  persist_event_types: string[];
  retention_days: number;
  rules: AlertRule[];
  notify_config: NotifyConfig;
  db_path: string;
  enable_sni: boolean;
  enable_dns_pcap: boolean;
  load_limit: number;
  max_size_mb: number;
}

const EVENT_TYPE_OPTIONS = [
  { key: "dns", label: "DNS查询" },
  { key: "dns_client", label: "DNS-Client" },
  { key: "network_connect", label: "网络连接" },
  { key: "network_monitor", label: "网络监控" },
  { key: "tls_sni", label: "TLS SNI" },
  { key: "dns_pcap", label: "DNS抓包" },
  { key: "create_remote_thread", label: "远程线程" },
  { key: "file_create", label: "文件创建" },
];

function createEmptyRule(): AlertRule {
  return {
    id: crypto.randomUUID(),
    name: "",
    targets: [],
    event_types: [],
    enabled: true,
  };
}

type SettingsTab = "alert-rules" | "notification" | "database" | "data-source" | "import-export";

export default function SettingsPage() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<SettingsTab>("alert-rules");
  const [config, setConfig] = useState<MonitorConfig>({
    background_mode: false,
    persist_event_types: [],
    retention_days: 7,
    rules: [],
    notify_config: {
      popup_rule_ids: [],
      feishu_rule_ids: [],
      feishu_webhook_url: "",
      popup_duration_secs: 10,
    },
    db_path: "",
    enable_sni: true,
    enable_dns_pcap: true,
    load_limit: 1000,
    max_size_mb: 512,
  });
  const [loading, setLoading] = useState(false);
  const [targetsInput, setTargetsInput] = useState<Record<string, string>>({});
  const [feishuWebhookUrl, setFeishuWebhookUrl] = useState("");
  const [revealedFeishuUrl, setRevealedFeishuUrl] = useState(false);
  const [testingFeishu, setTestingFeishu] = useState(false);


  useEffect(() => {
    invoke<MonitorConfig>("cmd_monitor_get_config").then((c) => {
      setConfig(c);
      const ti: Record<string, string> = {};
      c.rules.forEach((r) => {
        ti[r.id] = (r.targets || []).join(", ");
      });
      setTargetsInput(ti);
      setFeishuWebhookUrl(c.notify_config?.feishu_webhook_url ?? "");
    }).catch(() => {});
  }, []);

  const addRule = () => {
    setConfig((prev) => ({ ...prev, rules: [...prev.rules, createEmptyRule()] }));
  };

  const removeRule = (id: string) => {
    setConfig((prev) => ({ ...prev, rules: prev.rules.filter((r) => r.id !== id) }));
    setTargetsInput((prev) => {
      const next: Record<string, string> = {};
      Object.keys(prev).forEach((k) => {
        if (k !== id) next[k] = prev[k];
      });
      return next;
    });
  };

  const updateRule = (id: string, updates: Partial<AlertRule>) => {
    setConfig((prev) => ({
      ...prev,
      rules: prev.rules.map((r) => (r.id === id ? { ...r, ...updates } : r)),
    }));
  };

  const toggleEventType = (ruleId: string, eventType: string) => {
    const rule = config.rules.find((r) => r.id === ruleId);
    if (!rule) return;
    const types = rule.event_types.includes(eventType)
      ? rule.event_types.filter((t) => t !== eventType)
      : [...rule.event_types, eventType];
    updateRule(ruleId, { event_types: types });
  };

  const togglePopupRule = (ruleId: string) => {
    setConfig((prev) => {
      const ids = prev.notify_config.popup_rule_ids.includes(ruleId)
        ? prev.notify_config.popup_rule_ids.filter((id) => id !== ruleId)
        : [...prev.notify_config.popup_rule_ids, ruleId];
      return { ...prev, notify_config: { ...prev.notify_config, popup_rule_ids: ids } };
    });
  };

  const toggleFeishuRule = (ruleId: string) => {
    setConfig((prev) => {
      const ids = prev.notify_config.feishu_rule_ids.includes(ruleId)
        ? prev.notify_config.feishu_rule_ids.filter((id) => id !== ruleId)
        : [...prev.notify_config.feishu_rule_ids, ruleId];
      return { ...prev, notify_config: { ...prev.notify_config, feishu_rule_ids: ids } };
    });
  };

  const selectAllPopup = (select: boolean) => {
    setConfig((prev) => ({
      ...prev,
      notify_config: {
        ...prev.notify_config,
        popup_rule_ids: select ? prev.rules.map((r) => r.id) : [],
      },
    }));
  };

  const selectAllFeishu = (select: boolean) => {
    setConfig((prev) => ({
      ...prev,
      notify_config: {
        ...prev.notify_config,
        feishu_rule_ids: select ? prev.rules.map((r) => r.id) : [],
      },
    }));
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

  const testFeishu = async () => {
    if (!feishuWebhookUrl) {
      toast.error(t("log-collector.monitor.webhook-url-required"));
      return;
    }
    setTestingFeishu(true);
    try {
      await invoke("cmd_monitor_test_feishu", { webhookUrl: feishuWebhookUrl });
      toast.success(t("log-collector.monitor.test-feishu-success"));
    } catch (e) {
      toast.error(t("log-collector.monitor.test-feishu-failed"), { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setTestingFeishu(false);
    }
  };

  const handleExportAlertRules = async () => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      const filePath = await save({
        defaultPath: `irtool-alert-rules-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      await writeTextFile(filePath, JSON.stringify(config.rules, null, 2));
      toast.success(t("settings.import-export.export-success"));
    } catch {
      toast.error(t("settings.import-export.export-failed"));
    }
  };

  const handleImportAlertRules = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const { readTextFile } = await import("@tauri-apps/plugin-fs");
      const filePath = await open({
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      if (!filePath) return;
      const text = await readTextFile(filePath as string);
      const imported = JSON.parse(text);
      if (!Array.isArray(imported)) {
        toast.error(t("settings.import-export.import-failed"));
        return;
      }
      const existingIds = new Set(config.rules.map((r) => r.id));
      const newRules = imported.filter((r: any) => r.id && !existingIds.has(r.id));
      setConfig((prev) => ({ ...prev, rules: [...prev.rules, ...newRules] }));
      toast.success(t("settings.import-export.import-success"));
    } catch {
      toast.error(t("settings.import-export.import-failed"));
    }
  };

  const handleExport = async () => {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      const filePath = await save({
        defaultPath: `irtool-config-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!filePath) return;
      const monitorConfig = await invoke<MonitorConfig>("cmd_monitor_get_config");
      const { rules, ...appConfig } = monitorConfig;
      const data = { app_config: appConfig, exported_at: new Date().toISOString() };
      await writeTextFile(filePath, JSON.stringify(data, null, 2));
      toast.success(t("settings.import-export.export-success"));
    } catch {
      toast.error(t("settings.import-export.export-failed"));
    }
  };

  const handleImport = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const { readTextFile } = await import("@tauri-apps/plugin-fs");
      const filePath = await open({
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      if (!filePath) return;
      const text = await readTextFile(filePath as string);
      const data = JSON.parse(text);
      const configData = data.app_config || data.monitor_config;
      if (!configData) {
        toast.error(t("settings.import-export.import-failed"), { description: "文件格式无效：未找到配置数据" });
        return;
      }
      const currentRules = config.rules;
      const restoredConfig = { ...configData, rules: currentRules };
      await invoke("cmd_monitor_update_config", { config: restoredConfig as any });
      setConfig(restoredConfig as MonitorConfig);
      toast.success(t("settings.import-export.import-success"));
    } catch {
      toast.error(t("settings.import-export.import-failed"));
    }
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      const updatedRules = config.rules.map((r) => ({
        ...r,
        targets: (targetsInput[r.id] ?? "").split(",").map((s) => s.trim()).filter(Boolean),
      }));
      const updatedConfig = {
        ...config,
        rules: updatedRules,
        notify_config: {
          ...config.notify_config,
          feishu_webhook_url: feishuWebhookUrl,
        },
      };
      await invoke("cmd_monitor_update_config", { config: updatedConfig as any });
      toast.success(t("settings.save-success"));
    } catch (e) {
      toast.error(t("settings.save-failed"), { description: e instanceof Error ? e.message : "" });
    } finally {
      setLoading(false);
    }
  };

  const tabs: { key: SettingsTab; icon: typeof Bell; label: string }[] = [
    { key: "alert-rules", icon: ShieldAlert, label: t("settings.tabs.alert-rules") },
    { key: "notification", icon: Bell, label: t("settings.tabs.notification") },
    { key: "database", icon: Database, label: t("settings.tabs.database") },
    { key: "data-source", icon: Info, label: t("settings.tabs.data-source") },
    { key: "import-export", icon: Download, label: t("settings.tabs.import-export") },
  ];

  const getEventTypeLabels = (eventTypes: string[]) => {
    return eventTypes.map((et) => {
      const opt = EVENT_TYPE_OPTIONS.find((o) => o.key === et);
      return opt ? opt.label : et;
    });
  };

  return (
    <div className="flex h-full">
      {/* Left tab navigation */}
      <div className="w-40 border-r border-border bg-bg-elev-1 py-3 px-2">
        <p className="text-xs font-medium text-fg-secondary px-2 mb-2">{t("settings.title")}</p>
        <div className="space-y-0.5">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              className={`w-full text-left text-xs px-2 py-1.5 rounded-md flex items-center gap-1.5 ${
                activeTab === tab.key ? "bg-accent/10 text-accent" : "text-fg-secondary hover:bg-bg-elev-2/40"
              }`}
              onClick={() => setActiveTab(tab.key)}
            >
              <tab.icon className="h-3.5 w-3.5" />
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Right content area */}
      <div className="flex-1 overflow-y-auto p-4">
        <div className="max-w-2xl space-y-4">
          {activeTab === "alert-rules" && (
            <>
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-medium">{t("settings.alert-rules.title")}</h2>
                <div className="flex items-center gap-2">
                  <Button size="sm" onClick={handleSave} disabled={loading} className="hover:shadow-sm transition-shadow">
                    {loading ? t("log-collector.monitor.saving") : t("log-collector.monitor.save")}
                  </Button>
                  <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={addRule}>
                    <Plus className="h-3 w-3 mr-1" />
                    {t("settings.alert-rules.add")}
                  </Button>
                  <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={handleExportAlertRules}>
                    <Download className="h-3 w-3 mr-1" />
                    {t("workspace.rules.export")}
                  </Button>
                  <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={handleImportAlertRules}>
                    <Upload className="h-3 w-3 mr-1" />
                    {t("workspace.rules.import")}
                  </Button>
                </div>
              </div>

              {config.rules.length === 0 && (
                <p className="text-xs text-fg-tertiary py-8 text-center">{t("settings.alert-rules.empty")}</p>
              )}

              <div className="space-y-3">
                {config.rules.map((rule) => (
                  <div key={rule.id} className="border border-border rounded-md p-3 space-y-2">
                    <div className="flex items-center gap-2">
                      <Input
                        placeholder={t("settings.alert-rules.rule-name")}
                        value={rule.name}
                        onChange={(e) => updateRule(rule.id, { name: e.target.value })}
                        className="h-7 text-xs flex-1"
                      />
                      <Checkbox checked={rule.enabled} onCheckedChange={(v) => updateRule(rule.id, { enabled: v === true })} />
                      <Label className="text-[10px]">{rule.enabled ? t("settings.alert-rules.enabled") : t("settings.alert-rules.disabled")}</Label>
                      <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => removeRule(rule.id)}>
                        <Trash2 className="h-3 w-3 text-red-500" />
                      </Button>
                    </div>

                    <Input
                      placeholder={t("settings.alert-rules.targets-placeholder")}
                      value={targetsInput[rule.id] ?? ""}
                      onChange={(e) => setTargetsInput((prev) => ({ ...prev, [rule.id]: e.target.value }))}
                      className="h-7 text-xs"
                    />

                    <div className="flex flex-wrap gap-1.5">
                      {EVENT_TYPE_OPTIONS.map((et) => (
                        <Badge
                          key={et.key}
                          variant={rule.event_types.includes(et.key) ? "default" : "outline"}
                          className="text-[10px] cursor-pointer"
                          onClick={() => toggleEventType(rule.id, et.key)}
                        >
                          {et.label}
                        </Badge>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}

          {activeTab === "notification" && (
            <>
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-medium">{t("settings.notification.title")}</h2>
                <Button size="sm" onClick={handleSave} disabled={loading} className="hover:shadow-sm transition-shadow">
                  {loading ? t("log-collector.monitor.saving") : t("log-collector.monitor.save")}
                </Button>
              </div>

              {/* Popup notification section */}
              <div className="border border-border rounded-md p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-medium">{t("settings.notification.popup")}</h3>
                  <div className="flex items-center gap-2">
                    <Button variant="ghost" size="sm" className="h-5 text-[10px]" onClick={() => selectAllPopup(true)}>
                      {t("settings.notification.select-all")}
                    </Button>
                    <Button variant="ghost" size="sm" className="h-5 text-[10px]" onClick={() => selectAllPopup(false)}>
                      {t("settings.notification.deselect-all")}
                    </Button>
                  </div>
                </div>

                {config.rules.length === 0 ? (
                  <p className="text-[10px] text-fg-tertiary py-2">{t("settings.notification.no-rules")}</p>
                ) : (
                  <div className="space-y-1.5">
                    {config.rules.map((rule) => (
                      <div key={rule.id} className="flex items-center gap-2">
                        <Checkbox
                          checked={config.notify_config.popup_rule_ids.includes(rule.id)}
                          onCheckedChange={() => togglePopupRule(rule.id)}
                        />
                        <span className="text-xs flex-1">{rule.name || t("settings.alert-rules.rule-name")}</span>
                        <div className="flex flex-wrap gap-1">
                          {getEventTypeLabels(rule.event_types).map((label) => (
                            <Badge key={label} variant="outline" className="text-[9px] py-0 px-1">
                              {label}
                            </Badge>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
                <div className="flex items-center gap-2 pt-1 border-t border-border">
                  <Label className="text-xs shrink-0">{t("settings.notification.popup-duration")}</Label>
                  <Input
                    type="number"
                    min={0}
                    value={config.notify_config.popup_duration_secs}
                    onChange={(e) => {
                      const v = parseInt(e.target.value);
                      if (!isNaN(v) && v >= 0) setConfig((prev) => ({
                        ...prev,
                        notify_config: { ...prev.notify_config, popup_duration_secs: v },
                      }));
                    }}
                    className="h-7 w-16 text-xs"
                  />
                  <span className="text-xs text-fg-tertiary">{t("settings.notification.popup-duration-hint")}</span>
                </div>
              </div>

              {/* Feishu notification section */}
              <div className="border border-border rounded-md p-3 space-y-2">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-medium">{t("settings.notification.feishu")}</h3>
                  <div className="flex items-center gap-2">
                    <Button variant="ghost" size="sm" className="h-5 text-[10px]" onClick={() => selectAllFeishu(true)}>
                      {t("settings.notification.select-all")}
                    </Button>
                    <Button variant="ghost" size="sm" className="h-5 text-[10px]" onClick={() => selectAllFeishu(false)}>
                      {t("settings.notification.deselect-all")}
                    </Button>
                  </div>
                </div>

                {config.rules.length === 0 ? (
                  <p className="text-[10px] text-fg-tertiary py-2">{t("settings.notification.no-rules")}</p>
                ) : (
                  <div className="space-y-1.5">
                    {config.rules.map((rule) => (
                      <div key={rule.id} className="flex items-center gap-2">
                        <Checkbox
                          checked={config.notify_config.feishu_rule_ids.includes(rule.id)}
                          onCheckedChange={() => toggleFeishuRule(rule.id)}
                        />
                        <span className="text-xs flex-1">{rule.name || t("settings.alert-rules.rule-name")}</span>
                        <div className="flex flex-wrap gap-1">
                          {getEventTypeLabels(rule.event_types).map((label) => (
                            <Badge key={label} variant="outline" className="text-[9px] py-0 px-1">
                              {label}
                            </Badge>
                          ))}
                        </div>
                      </div>
                    ))}
                  </div>
                )}

                <div className="flex items-center gap-2 pt-2">
                  <div className="relative flex-1">
                    <Input
                      placeholder={t("settings.notification.webhook-url")}
                      value={revealedFeishuUrl ? feishuWebhookUrl : maskUrl(feishuWebhookUrl)}
                      onChange={(e) => {
                        if (revealedFeishuUrl) {
                          setFeishuWebhookUrl(e.target.value);
                        }
                      }}
                      onFocus={() => setRevealedFeishuUrl(true)}
                      className="h-7 text-xs pr-8"
                      readOnly={!revealedFeishuUrl}
                    />
                    <button
                      type="button"
                      className="absolute right-1.5 top-1/2 -translate-y-1/2 text-fg-tertiary hover:text-fg-primary"
                      onClick={() => setRevealedFeishuUrl((prev) => !prev)}
                    >
                      {revealedFeishuUrl ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                    </button>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 shrink-0"
                    onClick={testFeishu}
                    disabled={testingFeishu || !feishuWebhookUrl}
                  >
                    <Send className="h-3 w-3" />
                  </Button>
                </div>
              </div>
            </>
          )}

          {activeTab === "database" && (
            <>
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-medium">{t("settings.database.title")}</h2>
                <Button size="sm" onClick={handleSave} disabled={loading} className="hover:shadow-sm transition-shadow">
                  {loading ? t("log-collector.monitor.saving") : t("log-collector.monitor.save")}
                </Button>
              </div>
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <Label className="text-xs shrink-0 w-28">{t("settings.database.retention-days")}</Label>
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
                  <span className="text-xs text-fg-tertiary">{t("settings.database.retention-days-hint")}</span>
                </div>
                <div className="flex items-center gap-2">
                  <Label className="text-xs shrink-0 w-28">{t("settings.database.max-size")}</Label>
                  <Input
                    type="number"
                    min={0}
                    value={config.max_size_mb}
                    onChange={(e) => {
                      const v = parseInt(e.target.value);
                      if (!isNaN(v) && v >= 0) setConfig((prev) => ({ ...prev, max_size_mb: v }));
                    }}
                    className="h-7 w-20 text-xs"
                  />
                  <span className="text-xs text-fg-tertiary">MB{t("settings.database.max-size-hint")}</span>
                </div>
                <div className="flex items-center gap-2">
                  <Label className="text-xs shrink-0 w-28">{t("settings.database.load-limit")}</Label>
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
                  <span className="text-xs text-fg-tertiary">{t("settings.database.load-limit-hint")}</span>
                </div>
                <div className="flex items-center gap-2">
                  <Label className="text-xs shrink-0 w-28">{t("settings.database.db-path")}</Label>
                  <Input
                    type="text"
                    placeholder={t("settings.database.db-path-placeholder")}
                    value={config.db_path}
                    onChange={(e) => setConfig((prev) => ({ ...prev, db_path: e.target.value }))}
                    className="h-7 text-xs flex-1"
                  />
                </div>
              </div>
            </>
          )}

          {activeTab === "data-source" && (
            <>
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-medium">{t("settings.data-source.title")}</h2>
                <Button size="sm" onClick={handleSave} disabled={loading} className="hover:shadow-sm transition-shadow">
                  {loading ? t("log-collector.monitor.saving") : t("log-collector.monitor.save")}
                </Button>
              </div>
              <div className="space-y-4">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <Checkbox
                      checked={config.enable_sni}
                      onCheckedChange={(v) => setConfig((prev) => ({ ...prev, enable_sni: v === true }))}
                    />
                    <Label className="text-xs">{t("settings.data-source.sni")}</Label>
                  </div>
                  <p className="text-[10px] text-fg-tertiary pl-6 leading-relaxed">
                    {t("settings.data-source.sni-desc")}
                  </p>
                  <p className="text-[10px] text-fg-tertiary pl-6">
                    {t("settings.data-source.sni-source")}
                  </p>
                </div>
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <Checkbox
                      checked={config.enable_dns_pcap}
                      onCheckedChange={(v) => setConfig((prev) => ({ ...prev, enable_dns_pcap: v === true }))}
                    />
                    <Label className="text-xs">{t("settings.data-source.dns-pcap")}</Label>
                  </div>
                  <p className="text-[10px] text-fg-tertiary pl-6 leading-relaxed">
                    {t("settings.data-source.dns-pcap-desc")}
                  </p>
                  <p className="text-[10px] text-fg-tertiary pl-6">
                    {t("settings.data-source.dns-pcap-source")}
                  </p>
                </div>
              </div>
            </>
          )}

          {activeTab === "import-export" && (
            <>
              <h2 className="text-sm font-medium">{t("settings.import-export.title")}</h2>
              <p className="text-xs text-fg-tertiary">{t("settings.import-export.desc")}</p>
              <div className="flex gap-3 pt-2">
                <Button variant="secondary" size="sm" onClick={handleExport} className="hover:shadow-sm transition-shadow">
                  <Download className="h-3.5 w-3.5 mr-1" />
                  {t("settings.import-export.export")}
                </Button>
                <Button variant="secondary" size="sm" onClick={handleImport} className="hover:shadow-sm transition-shadow">
                  <Upload className="h-3.5 w-3.5 mr-1" />
                  {t("settings.import-export.import")}
                </Button>
              </div>
            </>
          )}


        </div>
      </div>
    </div>
  );
}
