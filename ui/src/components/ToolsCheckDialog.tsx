import { useState, useEffect, useCallback } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Download, FolderOpen, CheckCircle, AlertCircle, RotateCcw } from "lucide-react";
import type { ToolStatus } from "@/lib/bindings";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type DownloadState = "idle" | "downloading" | "done" | "error";

const TOOL_LABELS: Record<string, string> = {
  autoruns: "Autoruns",
  sigcheck: "Sigcheck",
  sysmon: "Sysmon",
};

function stringifyError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export function ToolsCheckDialog({ open, onOpenChange }: Props) {
  const [tools, setTools] = useState<ToolStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [downloadState, setDownloadState] = useState<DownloadState>("idle");
  const [progressMap, setProgressMap] = useState<Record<string, number>>({});
  const [errorMsg, setErrorMsg] = useState("");

  const refreshTools = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<ToolStatus[]>("cmd_tools_check");
      setTools(result);
    } catch {
      setTools([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) {
      refreshTools();
      setDownloadState("idle");
      setProgressMap({});
      setErrorMsg("");
    }
  }, [open, refreshTools]);

  useEffect(() => {
    if (!open) return;

    const unlistenProgress = listen<{ tool_id: string; downloaded: number; total: number }>(
      "evt_tools_download_progress",
      (event) => {
        const { tool_id, downloaded, total } = event.payload;
        if (total > 0) {
          setProgressMap((prev) => ({
            ...prev,
            [tool_id]: Math.round((downloaded / total) * 100),
          }));
        }
      },
    );

    const unlistenComplete = listen("evt_tools_download_complete", () => {
      setDownloadState("done");
      refreshTools();
    });

    const unlistenError = listen<{ tool_id: string; error: string }>(
      "evt_tools_download_error",
      (event) => {
        setErrorMsg((prev) => {
          const entry = `${TOOL_LABELS[event.payload.tool_id] || event.payload.tool_id}: ${event.payload.error}`;
          return prev ? `${prev}\n${entry}` : entry;
        });
      },
    );

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
      unlistenError.then((fn) => fn());
    };
  }, [open, refreshTools]);

  const missingTools = tools.filter((t) => !t.installed);
  const allInstalled = tools.length > 0 && missingTools.length === 0;

  // Overall progress across all downloading tools
  const progressEntries = Object.values(progressMap);
  const overallProgress = progressEntries.length > 0
    ? Math.round(progressEntries.reduce((a, b) => a + b, 0) / progressEntries.length)
    : 0;

  const handleDownload = async () => {
    const ids = missingTools.map((t) => t.id);
    if (ids.length === 0) return;
    setDownloadState("downloading");
    setProgressMap({});
    setErrorMsg("");
    try {
      await invoke("cmd_tools_download", { toolIds: ids });
    } catch (e) {
      // Individual tool errors are handled via evt_tools_download_error events
      if (!errorMsg) {
        setErrorMsg(stringifyError(e));
      }
    }
  };

  const handleImportZip = async (toolId: string) => {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const filePath = await openDialog({
        filters: [{ name: "ZIP", extensions: ["zip"] }],
        multiple: false,
      });
      if (!filePath) return;
      setDownloadState("downloading");
      await invoke("cmd_tools_import_zip", { toolId, zipPath: filePath });
      setDownloadState("done");
      refreshTools();
    } catch (e) {
      setDownloadState("error");
      setErrorMsg(stringifyError(e));
    }
  };

  const handleRelaunch = async () => {
    if (import.meta.env.DEV) {
      // In dev mode, relaunch kills the vite dev server.
      // Tools are already on disk, just close the dialog.
      onOpenChange(false);
    } else {
      // 使用自定义 cmd_relaunch 而非 plugin-process 的 relaunch()，
      // 因为后者在便携版下会被单实例互斥锁拦截导致新进程立即退出。
      try {
        await invoke("cmd_relaunch");
      } catch (e) {
        setErrorMsg(stringifyError(e));
      }
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>外部工具管理</DialogTitle>
          <DialogDescription>
            以下工具需要从 Microsoft 官方下载，IRTool 不会内置或二次分发这些二进制文件。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {tools.map((tool) => (
            <div
              key={tool.id}
              className="flex items-center justify-between gap-3 p-2 rounded-md border border-border"
            >
              <div className="flex items-center gap-2 min-w-0">
                {tool.installed ? (
                  <CheckCircle className="h-4 w-4 text-success shrink-0" />
                ) : (
                  <AlertCircle className="h-4 w-4 text-warning shrink-0" />
                )}
                <div className="min-w-0">
                  <div className="text-sm font-medium">
                    {TOOL_LABELS[tool.id] || tool.id}
                    {tool.optional && (
                      <span className="ml-1.5 text-[10px] font-normal text-fg-tertiary border border-border rounded px-1">可选</span>
                    )}
                  </div>
                  <div className="text-[10px] text-fg-tertiary truncate">
                    {tool.installed
                      ? `v${tool.version} — ${tool.files.join(", ")}`
                      : `缺失: ${tool.missing_files.join(", ")}`}
                  </div>
                </div>
              </div>
              {!tool.installed && downloadState !== "downloading" && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-6 text-xs shrink-0"
                  onClick={() => handleImportZip(tool.id)}
                >
                  <FolderOpen className="h-3 w-3 mr-1" />
                  导入ZIP
                </Button>
              )}
            </div>
          ))}

          {loading && <div className="text-xs text-fg-tertiary text-center">检测中...</div>}

          {downloadState === "downloading" && (
            <div className="space-y-2">
              <div className="text-xs text-fg-secondary">
                正在下载 {Object.keys(progressMap).map((id) => TOOL_LABELS[id] || id).join(", ")}...
              </div>
              <Progress value={overallProgress} className="h-2" />
              <div className="text-[10px] text-fg-tertiary text-right">{overallProgress}%</div>
            </div>
          )}

          {downloadState === "error" && (
            <div className="text-xs text-red-500 p-2 bg-red-500/10 rounded">
              下载失败: {errorMsg}
            </div>
          )}

          {downloadState === "done" && errorMsg && (
            <div className="text-xs text-red-500 p-2 bg-red-500/10 rounded whitespace-pre-line">
              部分工具下载失败: {errorMsg}
            </div>
          )}

          {downloadState === "done" && !errorMsg && (
            <div className="text-xs text-green-500 p-2 bg-green-500/10 rounded">
              下载完成，EULA 已自动接受。点击"立即重启生效"以加载新工具。
            </div>
          )}
        </div>

        <div className="flex items-center justify-between gap-2 pt-2">
          <div className="text-[10px] text-fg-tertiary">
            来源: download.sysinternals.com (Microsoft 官方)
          </div>
          <div className="flex gap-2">
            {downloadState === "done" && (
              <Button variant="default" size="sm" onClick={handleRelaunch}>
                <RotateCcw className="h-3 w-3 mr-1" />
                {import.meta.env.DEV ? "完成" : "立即重启生效"}
              </Button>
            )}
            <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)}>
              {allInstalled ? "关闭" : "稍后"}
            </Button>
            {missingTools.length > 0 && downloadState !== "downloading" && downloadState !== "done" && (
              <Button variant="default" size="sm" onClick={handleDownload}>
                <Download className="h-3 w-3 mr-1" />
                一键下载缺失工具
              </Button>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
