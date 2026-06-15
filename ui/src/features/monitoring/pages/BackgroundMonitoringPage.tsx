import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Play,
  Square,
  Database,
  HardDrive,
  Cpu,
  Wifi,
  Shield,
  Settings,
  Zap,
  AlertTriangle,
} from "lucide-react";
import { toast } from "sonner";
import * as api from "../api";
import type { MonitorConfig } from "../api";
import { useMonitoringStore } from "../store";
import { formatUptime, formatTimestamp } from "../utils";

// --- 格式化字节数 ---
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(val >= 100 ? 0 : 1)} ${units[i]}`;
}

// --- Preset definitions ---
interface Preset {
  label: string;
  description: string;
  config: Partial<MonitorConfig>;
}

const SYSMON_EVENT_GROUPS = [
  { key: "dns", label: "DNS 查询" },
  { key: "network_connect", label: "网络连接" },
  { key: "create_remote_thread", label: "远程线程" },
  { key: "file_create", label: "文件创建" },
  { key: "tls_sni", label: "TLS SNI" },
  { key: "dns_pcap", label: "DNS 抓包" },
];

export default function BackgroundMonitoringPage() {
  const { t } = useTranslation();
  const { isBackground, telemetry, eventCount, dbSize, setIsBackground, setTelemetry, setEventCount, setDbSize } =
    useMonitoringStore();

  const [config, setConfig] = useState<MonitorConfig>({
    background_mode: false,
    persist_event_types: [],
    retention_days: 7,
    rules: [],
    db_path: "",
    enable_sni: true, // 默认勾选 TLS SNI 提取
    enable_dns_pcap: true, // 默认勾选网络层 DNS 抓包
    adapter_ip: null,
    max_duration_secs: 0,
    load_limit: 1000,
    max_size_mb: 512,
    notify_config: {
      popup_rule_ids: [],
      feishu_rule_ids: [],
      feishu_webhook_url: "",
      popup_duration_secs: 10,
    },
  });
  const [confirmDialogOpen, setConfirmDialogOpen] = useState(false);
  const [dnsSniDialogOpen, setDnsSniDialogOpen] = useState(false);
  const [saving, setSaving] = useState(false);

  // Load config on mount
  useEffect(() => {
    api.getMonitorConfig().then(setConfig).catch(() => {});
  }, []);

  // Poll telemetry
  useEffect(() => {
    let mounted = true;
    const poll = async () => {
      try {
        const [tel, bg, count, size] = await Promise.all([
          api.getTelemetry(),
          api.isBackground(),
          api.getEventCount(),
          api.getDbSize(),
        ]);
        if (!mounted) return;
        setTelemetry({
          mode: tel.mode,
          started_at: tel.started_at,
          events_written: tel.events_written,
          events_dropped: tel.events_dropped,
          last_event_at: tel.last_event_at,
          last_error: tel.last_error,
        });
        setIsBackground(bg);
        setEventCount(count);
        setDbSize(size);
      } catch {}
    };
    poll();
    const interval = setInterval(poll, 3000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, [setTelemetry, setIsBackground, setEventCount, setDbSize]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    try {
      await api.updateMonitorConfig(config);
      toast.success(t("settings.save-success"));
    } catch (e) {
      toast.error(t("settings.save-failed"), {
        description: e instanceof Error ? e.message : "",
      });
    } finally {
      setSaving(false);
    }
  }, [config, t]);

  const handleEnterBackground = useCallback(async () => {
    try {
      await api.enterBackground();
      setIsBackground(true);
      setConfirmDialogOpen(false);
    } catch (e) {
      toast.error("进入后台失败", { description: e instanceof Error ? e.message : "" });
    }
  }, [setIsBackground]);

  const handleExitBackground = useCallback(async () => {
    try {
      await api.exitBackground();
      setIsBackground(false);
    } catch (e) {
      toast.error("退出后台失败", { description: e instanceof Error ? e.message : "" });
    }
  }, [setIsBackground]);

  const togglePersistType = useCallback((key: string) => {
    setConfig((prev) => ({
      ...prev,
      persist_event_types: prev.persist_event_types.includes(key)
        ? prev.persist_event_types.filter((t) => t !== key)
        : [...prev.persist_event_types, key],
    }));
  }, []);

  // --- Presets ---
  const presets: Preset[] = [
    {
      label: "低开销",
      description: "Sysmon DNS + 网络连接，网络快照 5s，PCAP 关闭，仅持久化外连/DNS 事件",
      config: {
        persist_event_types: ["dns", "network_connect"],
        enable_sni: false,
        enable_dns_pcap: false,
      },
    },
    {
      label: "均衡",
      description: "DNS + 网络连接 + 远程线程 + 文件创建，PCAP DNS/SNI 关闭",
      config: {
        persist_event_types: ["dns", "network_connect", "create_remote_thread", "file_create"],
        enable_sni: false,
        enable_dns_pcap: false,
      },
    },
    {
      label: "深度捕获",
      description: "均衡 + PCAP DNS/SNI，更完整的持久化覆盖",
      config: {
        persist_event_types: ["dns", "network_connect", "create_remote_thread", "file_create", "tls_sni", "dns_pcap", "network_monitor"],
        enable_sni: true,
        enable_dns_pcap: true,
      },
    },
  ];

  const applyPreset = useCallback((preset: Preset) => {
    setConfig((prev) => ({ ...prev, ...preset.config }));
    toast.success(`已应用预设: ${preset.label}`);
  }, []);

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto p-4">
        <div className="max-w-3xl space-y-4">
          {/* Status Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Cpu className="h-4 w-4 text-fg-secondary" />
                <h2 className="text-sm font-medium">运行状态</h2>
              </div>
              <div className="flex items-center gap-2">
                {isBackground ? (
                  <Button variant="destructive" size="sm" onClick={handleExitBackground} className="h-7 text-xs border-2 border-red-500">
                    <Square className="h-3 w-3 mr-1" />
                    退出后台模式
                  </Button>
                ) : (
                  <Button size="sm" onClick={() => setConfirmDialogOpen(true)} className="h-7 text-xs">
                    <Play className="h-3 w-3 mr-1" />
                    进入后台模式
                  </Button>
                )}
              </div>
            </div>

            <div className="grid grid-cols-4 gap-3">
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">模式</span>
                <Badge variant={isBackground ? "default" : "outline"} className="text-[10px] w-fit">
                  {isBackground ? "后台运行" : "前台"}
                </Badge>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">运行时长</span>
                <span className="text-xs font-mono">{formatUptime(telemetry?.started_at ?? null)}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">事件写入</span>
                <span className="text-xs font-mono">{telemetry?.events_written ?? 0}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">事件丢弃</span>
                <span className="text-xs font-mono">{telemetry?.events_dropped ?? 0}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">最后事件</span>
                <span className="text-xs font-mono">{formatTimestamp(telemetry?.last_event_at ?? null)}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">数据库记录</span>
                <span className="text-xs font-mono">{eventCount}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">数据库大小</span>
                <span className="text-xs font-mono">{formatBytes(dbSize)}</span>
              </div>
              <div className="flex flex-col gap-0.5">
                <span className="text-[10px] text-fg-tertiary">最后错误</span>
                <span className="text-xs text-red-400 truncate" title={telemetry?.last_error ?? undefined}>
                  {telemetry?.last_error || "-"}
                </span>
              </div>
            </div>
          </section>

          {/* Storage Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center gap-2">
              <Database className="h-4 w-4 text-fg-secondary" />
              <h2 className="text-sm font-medium">存储配置</h2>
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label className="text-xs shrink-0 w-28">保留天数</Label>
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
                <span className="text-xs text-fg-tertiary">0 = 永久保留</span>
              </div>
              <div className="flex items-center gap-2">
                <Label className="text-xs shrink-0 w-28">大小限制</Label>
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
                <span className="text-xs text-fg-tertiary">MB，0 = 不限制</span>
              </div>
              <div className="flex items-center gap-2">
                <Label className="text-xs shrink-0 w-28">存储路径</Label>
                <Input
                  type="text"
                  placeholder="留空使用默认路径"
                  value={config.db_path}
                  onChange={(e) => setConfig((prev) => ({ ...prev, db_path: e.target.value }))}
                  className="h-7 text-xs flex-1"
                />
              </div>
              <div className="flex items-center gap-2 pt-1">
                <Button variant="secondary" size="sm" className="h-7 text-xs" onClick={handleSave} disabled={saving}>
                  <HardDrive className="h-3 w-3 mr-1" />
                  {saving ? "保存中..." : "保存配置"}
                </Button>
              </div>
            </div>
          </section>

          {/* 日志采集事件 Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Shield className="h-4 w-4 text-fg-secondary" />
                <h2 className="text-sm font-medium">日志采集事件</h2>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 text-[10px] gap-1 px-1.5"
                onClick={() => setDnsSniDialogOpen(true)}
              >
                <Settings className="h-3 w-3" />
                配置
              </Button>
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-1 mb-1">
                <span className="text-[10px] text-fg-tertiary mr-1">持久化事件类型:</span>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {SYSMON_EVENT_GROUPS.map((g) => (
                  <Badge
                    key={g.key}
                    variant={config.persist_event_types.includes(g.key) ? "default" : "outline"}
                    className="text-[10px] cursor-pointer"
                    onClick={() => togglePersistType(g.key)}
                  >
                    {g.label}
                  </Badge>
                ))}
              </div>
            </div>
          </section>

          {/* Network Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center gap-2">
              <Wifi className="h-4 w-4 text-fg-secondary" />
              <h2 className="text-sm font-medium">网络监控</h2>
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label className="text-xs shrink-0 w-28">快照间隔</Label>
                <Input
                  type="number"
                  min={1}
                  max={60}
                  value={Math.round(config.max_duration_secs / 1000) || 2}
                  onChange={(e) => {
                    const v = parseInt(e.target.value);
                    if (!isNaN(v) && v >= 1) setConfig((prev) => ({ ...prev, max_duration_secs: v * 1000 }));
                  }}
                  className="h-7 w-20 text-xs"
                />
                <span className="text-xs text-fg-tertiary">秒</span>
              </div>
              <div className="flex items-center gap-2">
                <Label className="text-xs shrink-0 w-28">命令行富化</Label>
                <Select
                  value={config.adapter_ip ? "background" : "off"}
                  onValueChange={(v) =>
                    setConfig((prev) => ({ ...prev, adapter_ip: v === "off" ? null : prev.adapter_ip || "auto" }))
                  }
                >
                  <SelectTrigger className="h-7 w-32 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="off">关闭</SelectItem>
                    <SelectItem value="background">后台富化</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
          </section>

          {/* Rules and Notifications Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center gap-2">
              <Zap className="h-4 w-4 text-fg-secondary" />
              <h2 className="text-sm font-medium">规则与通知</h2>
            </div>
            {config.rules.length === 0 ? (
              <p className="text-xs text-fg-tertiary py-2">
                暂无监控规则，请在 设置 → 告警规则 中添加
              </p>
            ) : (
              <div className="space-y-2">
                {config.rules.map((rule) => (
                  <div key={rule.id} className="flex items-center gap-2 text-xs">
                    <Badge variant={rule.enabled ? "default" : "outline"} className="text-[10px]">
                      {rule.enabled ? "启用" : "禁用"}
                    </Badge>
                    <span className="flex-1 truncate">{rule.name || "未命名规则"}</span>
                    <div className="flex gap-1">
                      {rule.event_types.slice(0, 3).map((et) => (
                        <Badge key={et} variant="outline" className="text-[9px] py-0 px-1">
                          {et}
                        </Badge>
                      ))}
                      {rule.event_types.length > 3 && (
                        <span className="text-[9px] text-fg-tertiary">+{rule.event_types.length - 3}</span>
                      )}
                    </div>
                    <div className="flex gap-1">
                      {config.notify_config.popup_rule_ids.includes(rule.id) && (
                        <Badge variant="outline" className="text-[9px] py-0 px-1">弹窗</Badge>
                      )}
                      {config.notify_config.feishu_rule_ids.includes(rule.id) && (
                        <Badge variant="outline" className="text-[9px] py-0 px-1">飞书</Badge>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
            <div className="flex items-center gap-2 pt-1 border-t border-border">
              <Label className="text-xs shrink-0">弹窗时长</Label>
              <Input
                type="number"
                min={0}
                value={config.notify_config.popup_duration_secs}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  if (!isNaN(v) && v >= 0)
                    setConfig((prev) => ({
                      ...prev,
                      notify_config: { ...prev.notify_config, popup_duration_secs: v },
                    }));
                }}
                className="h-7 w-16 text-xs"
              />
              <span className="text-xs text-fg-tertiary">秒，0 = 不自动关闭</span>
            </div>
          </section>

          {/* Presets Section */}
          <section className="border border-border rounded-md p-3 space-y-3">
            <div className="flex items-center gap-2">
              <Zap className="h-4 w-4 text-fg-secondary" />
              <h2 className="text-sm font-medium">快速预设</h2>
            </div>
            <div className="grid grid-cols-3 gap-2">
              {presets.map((preset) => (
                <button
                  key={preset.label}
                  className="border border-border rounded-md p-2.5 text-left hover:border-accent/50 hover:bg-accent/5 transition-colors"
                  onClick={() => applyPreset(preset)}
                >
                  <div className="text-xs font-medium mb-1">{preset.label}</div>
                  <div className="text-[10px] text-fg-tertiary leading-relaxed">{preset.description}</div>
                </button>
              ))}
            </div>
            <div className="flex items-center gap-2 pt-1">
              <Button size="sm" onClick={handleSave} disabled={saving} className="h-7 text-xs">
                {saving ? "保存中..." : "应用并保存"}
              </Button>
              <span className="text-[10px] text-fg-tertiary">预设仅修改配置，需保存后生效</span>
            </div>
          </section>
        </div>
      </div>

      {/* DNS/SNI Config Dialog */}
      <Dialog open={dnsSniDialogOpen} onOpenChange={setDnsSniDialogOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>DNS / SNI 配置</DialogTitle>
            <DialogDescription>
              额外的网络层采集选项，开销较高，请按需开启
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3 py-2">
            <div className="flex items-center gap-2">
              <Checkbox
                checked={config.enable_dns_pcap}
                onCheckedChange={(v) => setConfig((prev) => ({ ...prev, enable_dns_pcap: v === true }))}
              />
              <Label className="text-xs">PCAP DNS 抓包</Label>
              <Badge variant="outline" className="text-[9px] text-amber-500 border-amber-500/30">
                <AlertTriangle className="h-2.5 w-2.5 mr-0.5" />
                较高开销
              </Badge>
            </div>
            <p className="text-[10px] text-fg-tertiary pl-6">
              从网卡层捕获 UDP:53 DNS 报文，覆盖 Go/dig 等不走系统 API 的工具
            </p>
            <div className="flex items-center gap-2">
              <Checkbox
                checked={config.enable_sni}
                onCheckedChange={(v) => setConfig((prev) => ({ ...prev, enable_sni: v === true }))}
              />
              <Label className="text-xs">TLS SNI 提取</Label>
              <Badge variant="outline" className="text-[9px] text-amber-500 border-amber-500/30">
                <AlertTriangle className="h-2.5 w-2.5 mr-0.5" />
                较高开销
              </Badge>
            </div>
            <p className="text-[10px] text-fg-tertiary pl-6">
              从 TLS 握手中提取域名，覆盖浏览器 DoH 等场景
            </p>
          </div>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setDnsSniDialogOpen(false)}>
              关闭
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Enter Background Confirm Dialog */}
      <Dialog open={confirmDialogOpen} onOpenChange={setConfirmDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>进入后台监控模式</DialogTitle>
            <DialogDescription>
              进入后台模式后，主窗口将隐藏到托盘，前端不会实时显示新事件，但数据采集和告警功能会继续运行，事件会持久化到
              SQLite 数据库中。
              <br />
              <br />
              点击托盘图标可以恢复窗口查看数据。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setConfirmDialogOpen(false)}>
              取消
            </Button>
            <Button onClick={handleEnterBackground}>确认进入</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
