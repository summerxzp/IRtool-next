import { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Activity, Play, Square, Rocket, AlertCircle, RotateCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import * as api from "../api";

/// CDP 抓包服务状态机：
/// - idle: 未运行，未探测端口
/// - probing: 正在探测端口
/// - no-port: 探测完成，无调试端口（显示"启动调试浏览器"按钮）
/// - ready: 探测到端口，未启动抓包（显示"启动抓包"按钮）
/// - starting: 正在启动抓包
/// - running: 抓包中（显示"停止"按钮 + 端口信息）
/// - stopping: 正在停止
/// - error: 出错（显示错误信息）
type State =
  | { kind: "idle" }
  | { kind: "probing" }
  | { kind: "no-port" }
  | { kind: "ready"; port: number; browser: string }
  | { kind: "starting" }
  | { kind: "running"; port: number; browser: string }
  | { kind: "stopping" }
  | { kind: "error"; message: string };

export function CdpCaptureControl() {
  const { t } = useTranslation();
  const [state, setState] = useState<State>({ kind: "idle" });
  const [launchingBrowser, setLaunchingBrowser] = useState(false);

  /// 初始探测 + 定期状态查询
  const refresh = useCallback(async () => {
    try {
      // 先查抓包服务状态
      const captureStatus = await api.cdpCaptureStatus();
      if (captureStatus.running) {
        setState({
          kind: "running",
          port: captureStatus.port ?? 9222,
          browser: captureStatus.browser_kind ?? "chrome",
        });
        return;
      }
      // 未运行 → 探测端口是否可用
      const probe = await api.cdpProbe();
      if (probe) {
        setState({
          kind: "ready",
          port: probe.port ?? 9222,
          browser: probe.browser_kind ?? "chrome",
        });
      } else {
        setState({ kind: "no-port" });
      }
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, []);

  useEffect(() => {
    refresh();
    // 5s 轮询：检测外部启动的调试浏览器（用户可能在 IRtool 外手动启动浏览器）。
    // 关键：轮询时不强制切到 probing 状态，避免 no-port ↔ probing UI 闪烁。
    // running/starting/stopping/launching 状态由按钮显式触发，不轮询。
    const id = setInterval(() => {
      if (launchingBrowser) return;
      if (
        state.kind === "running" ||
        state.kind === "starting" ||
        state.kind === "stopping"
      ) {
        return;
      }
      refresh();
    }, 5000);
    return () => clearInterval(id);
  }, [refresh, launchingBrowser, state.kind]);

  const handleStart = useCallback(async () => {
    setState({ kind: "starting" });
    try {
      const status = await api.cdpCaptureStart();
      setState({
        kind: "running",
        port: status.port ?? 9222,
        browser: status.browser_kind ?? "chrome",
      });
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, []);

  const handleStop = useCallback(async () => {
    setState({ kind: "stopping" });
    try {
      await api.cdpCaptureStop();
      setState({ kind: "idle" });
      // 停止后立即重新探测，回到 ready 或 no-port
      refresh();
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, [refresh]);

  const handleLaunchBrowser = useCallback(async () => {
    setLaunchingBrowser(true);
    try {
      // 后端会阻塞直到端口 9222 监听或超时（8s），无需前端 setTimeout
      await api.launchBrowserWithDebugPort("chrome");
      // 后端返回成功 = 端口已就绪，立即刷新到 ready 状态
      await refresh();
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    } finally {
      setLaunchingBrowser(false);
    }
  }, [refresh]);

  // ── 渲染 ──────────────────────────────────────────

  const running = state.kind === "running";
  const busy = state.kind === "starting" || state.kind === "stopping";
  const statusColor = running ? "bg-success" : state.kind === "error" ? "bg-danger" : "bg-muted-foreground/40";
  const statusText = (() => {
    switch (state.kind) {
      case "idle":
      case "probing":
        return t("browser-forensics.cdp.status-idle", { defaultValue: "CDP 待机" });
      case "no-port":
        return t("browser-forensics.cdp.status-no-port", { defaultValue: "无调试端口" });
      case "ready":
        return t("browser-forensics.cdp.status-ready", { defaultValue: "端口就绪" });
      case "starting":
        return t("browser-forensics.cdp.status-starting", { defaultValue: "启动中..." });
      case "running":
        return t("browser-forensics.cdp.status-running", { defaultValue: "抓包中" });
      case "stopping":
        return t("browser-forensics.cdp.status-stopping", { defaultValue: "停止中..." });
      case "error":
        return t("browser-forensics.cdp.status-error", { defaultValue: "错误" });
    }
  })();

  return (
    <span className="inline-flex items-center gap-1.5 h-8 select-none" role="status" aria-label={statusText}>
      <Activity className="h-3.5 w-3.5 text-muted-foreground" />
      <span className={`h-2 w-2 rounded-full shrink-0 ${statusColor}`} />
      <span className="text-xs text-muted-foreground">{statusText}</span>
      {running && state.kind === "running" && (
        <span className="text-[10px] text-muted-foreground/70 font-mono">
          :{state.port}
        </span>
      )}

      {/* 启动/停止按钮 */}
      {(state.kind === "ready" || state.kind === "running") && (
        <Button
          variant={running ? "secondary" : "default"}
          size="sm"
          className="h-6 px-2 text-[10px] gap-1"
          onClick={running ? handleStop : handleStart}
          disabled={busy}
        >
          {running ? (
            <>
              <Square className="h-3 w-3" />
              {t("browser-forensics.cdp.stop", { defaultValue: "停止" })}
            </>
          ) : (
            <>
              <Play className="h-3 w-3" />
              {t("browser-forensics.cdp.start", { defaultValue: "抓包" })}
            </>
          )}
        </Button>
      )}

      {/* 一键启动调试浏览器（no-port 时显示） */}
      {state.kind === "no-port" && (
        <Button
          variant="default"
          size="sm"
          className="h-6 px-2 text-[10px] gap-1"
          onClick={handleLaunchBrowser}
          disabled={launchingBrowser}
        >
          <Rocket className="h-3 w-3" />
          {launchingBrowser
            ? t("browser-forensics.cdp.launching", { defaultValue: "启动中..." })
            : t("browser-forensics.cdp.launch-browser", { defaultValue: "启动调试浏览器" })}
        </Button>
      )}

      {/* 错误提示：点击展开完整多行错误信息 + 重试按钮 */}
      {state.kind === "error" && (
        <Popover>
          <PopoverTrigger asChild>
            <button
              className="inline-flex items-center gap-1 h-6 px-1.5 text-[10px] text-danger hover:text-danger/80 rounded"
              title={t("browser-forensics.cdp.error-detail", { defaultValue: "点击查看详情" })}
            >
              <AlertCircle className="h-3 w-3" />
              {t("browser-forensics.cdp.status-error", { defaultValue: "错误" })}
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" sideOffset={4} className="w-80 p-3">
            <div className="space-y-2">
              <div className="text-[11px] font-medium text-fg-primary">
                {t("browser-forensics.cdp.error-title", { defaultValue: "启动失败" })}
              </div>
              <pre className="text-[10px] text-fg-secondary whitespace-pre-wrap break-words font-mono leading-relaxed max-h-48 overflow-y-auto">
                {state.message}
              </pre>
              <div className="flex justify-end gap-1.5 pt-1">
                <Button
                  variant="secondary"
                  size="sm"
                  className="h-6 px-2 text-[10px]"
                  onClick={() => setState({ kind: "idle" })}
                >
                  {t("browser-forensics.cdp.dismiss", { defaultValue: "关闭" })}
                </Button>
                <Button
                  variant="default"
                  size="sm"
                  className="h-6 px-2 text-[10px] gap-1"
                  onClick={() => {
                    setState({ kind: "idle" });
                    refresh();
                  }}
                >
                  <RotateCw className="h-3 w-3" />
                  {t("browser-forensics.cdp.retry", { defaultValue: "重试" })}
                </Button>
              </div>
            </div>
          </PopoverContent>
        </Popover>
      )}
    </span>
  );
}
