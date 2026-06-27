import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useBrowserForensicsStore } from "../store";
import * as api from "../api";

type Status = { kind: "success" | "error"; msg: string } | null;

export function WatchTargetsBar() {
  const { t } = useTranslation();
  const watchTargets = useBrowserForensicsStore((s) => s.watchTargets);
  const addWatchTarget = useBrowserForensicsStore((s) => s.addWatchTarget);
  const removeWatchTarget = useBrowserForensicsStore((s) => s.removeWatchTarget);
  const clearWatchTargets = useBrowserForensicsStore((s) => s.clearWatchTargets);
  const [input, setInput] = useState("");
  const [status, setStatus] = useState<Status>(null);
  const statusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 卸载时清理 setTimeout，避免对已卸载组件调用 setState
  useEffect(() => {
    return () => {
      if (statusTimerRef.current !== null) {
        clearTimeout(statusTimerRef.current);
      }
    };
  }, []);

  const setStatusWithTimeout = (s: NonNullable<Status>) => {
    if (statusTimerRef.current !== null) {
      clearTimeout(statusTimerRef.current);
    }
    setStatus(s);
    statusTimerRef.current = setTimeout(() => {
      statusTimerRef.current = null;
      setStatus(null);
    }, 2000);
  };

  const handleAdd = () => {
    const trimmed = input.trim();
    if (!trimmed) return;
    addWatchTarget(trimmed);
    setInput("");
  };

  const handleSend = async () => {
    let failed = false;
    await api.sendConfig(watchTargets, (msg) => {
      failed = true;
      setStatusWithTimeout({ kind: "error", msg });
    });
    if (!failed) {
      setStatusWithTimeout({
        kind: "success",
        msg: t("browser-forensics.watch.send-success", { defaultValue: "已下发到 Helper Extension" }),
      });
    }
  };

  const handleClear = async () => {
    let failed = false;
    // 先下发空配置（取消过滤），再清空本地列表
    await api.sendConfig([], (msg) => {
      failed = true;
      setStatusWithTimeout({ kind: "error", msg });
    });
    if (!failed) {
      clearWatchTargets();
      setStatusWithTimeout({
        kind: "success",
        msg: t("browser-forensics.watch.clear-success", { defaultValue: "已清除过滤" }),
      });
    }
  };

  return (
    <div className="px-3 py-1.5 border-b border-border bg-bg-elev-1/50">
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-xs font-medium text-muted-foreground select-none shrink-0">
          {t("browser-forensics.watch.label", { defaultValue: "关注目标" })}:
        </span>
        {/* 已添加的 chip 列表 */}
        {watchTargets.map((target) => (
          <span
            key={target}
            className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-accent/15 text-accent border border-accent/30"
          >
            <span className="font-mono select-none">{target}</span>
            <button
              className="text-accent/60 hover:text-danger"
              onClick={() => removeWatchTarget(target)}
              aria-label={`remove ${target}`}
            >
              ✕
            </button>
          </span>
        ))}
        {/* 输入框 */}
        <Input
          className="h-7 w-44 text-xs font-mono"
          placeholder={t("browser-forensics.watch.placeholder", { defaultValue: "域名/IP" })}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              handleAdd();
            }
          }}
        />
        {/* 添加按钮 */}
        <Button variant="secondary" size="sm" className="h-7 text-xs" onClick={handleAdd} disabled={!input.trim()}>
          {t("common.add", { defaultValue: "添加" })}
        </Button>
        <span className="mx-1 text-border select-none">|</span>
        {/* 下发按钮 */}
        <Button variant="default" size="sm" className="h-7 text-xs" onClick={handleSend} disabled={watchTargets.length === 0}>
          {t("browser-forensics.watch.send", { defaultValue: "下发到扩展" })}
        </Button>
        {/* 清除按钮 */}
        <Button variant="secondary" size="sm" className="h-7 text-xs" onClick={handleClear} disabled={watchTargets.length === 0}>
          {t("browser-forensics.watch.clear", { defaultValue: "清除" })}
        </Button>
        {/* 状态反馈 */}
        {status && (
          <span className={`text-xs ${status.kind === "success" ? "text-success" : "text-danger"}`}>
            {status.msg}
          </span>
        )}
      </div>
    </div>
  );
}
