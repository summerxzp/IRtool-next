import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle, AlertCircle, Copy, ExternalLink, Package, RefreshCw } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import * as api from "../api";
import type { BrowserKind } from "../types";
import type { ReconnectDiagnostics } from "../api";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type StepStatus = "idle" | "loading" | "success" | "error";

const BROWSERS: { kind: BrowserKind; label: string }[] = [
  { kind: "chrome", label: "Chrome" },
  { kind: "edge", label: "Edge" },
];

function errMsg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function InstallHelperExtensionDialog({ open, onOpenChange }: Props) {
  const { t } = useTranslation();
  const [browser, setBrowser] = useState<BrowserKind>("chrome");
  const [nmhStatus, setNmhStatus] = useState<StepStatus>("idle");
  const [nmhError, setNmhError] = useState("");
  const [pathStatus, setPathStatus] = useState<StepStatus>("idle");
  const [pathError, setPathError] = useState("");
  const [openPageStatus, setOpenPageStatus] = useState<StepStatus>("idle");
  const [openPageError, setOpenPageError] = useState("");
  const [reconnectStatus, setReconnectStatus] = useState<StepStatus>("idle");
  const [reconnectError, setReconnectError] = useState("");
  const [diagnostics, setDiagnostics] = useState<ReconnectDiagnostics | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [extensionIdOverride, setExtensionIdOverride] = useState("");

  // 重新打开时重置各步骤状态（保留浏览器选择）
  useEffect(() => {
    if (open) {
      setNmhStatus("idle");
      setNmhError("");
      setPathStatus("idle");
      setPathError("");
      setOpenPageStatus("idle");
      setOpenPageError("");
      setReconnectStatus("idle");
      setReconnectError("");
      setDiagnostics(null);
      setShowAdvanced(false);
      setExtensionIdOverride("");
    }
  }, [open]);

  const handleInstallNmh = async () => {
    const override = extensionIdOverride.trim();
    // 高级选项：如果填了 override，校验格式
    if (override && (override.length !== 32 || !/^[a-p]+$/.test(override))) {
      setNmhError(t("browser-forensics.install-helper.step-nmh-id-invalid"));
      setNmhStatus("error");
      return;
    }
    setNmhStatus("loading");
    setNmhError("");
    try {
      await api.installNativeMessagingHost(browser, override || undefined);
      setNmhStatus("success");
    } catch (e) {
      setNmhError(errMsg(e));
      setNmhStatus("error");
    }
  };

  const handleCopyPath = async () => {
    setPathStatus("loading");
    setPathError("");
    try {
      const path = await api.getHelperExtensionPath();
      await navigator.clipboard.writeText(path);
      setPathStatus("success");
    } catch (e) {
      setPathError(errMsg(e));
      setPathStatus("error");
    }
  };

  const handleOpenExtensionsPage = async () => {
    setOpenPageStatus("loading");
    setOpenPageError("");
    try {
      await api.openExtensionsPage(browser);
      setOpenPageStatus("success");
    } catch (e) {
      setOpenPageError(errMsg(e));
      setOpenPageStatus("error");
    }
  };

  const handleReconnect = async () => {
    setReconnectStatus("loading");
    setReconnectError("");
    setDiagnostics(null);
    try {
      const result = await api.reconnectExtension();
      setDiagnostics(result);
      setReconnectStatus("success");
    } catch (e) {
      setReconnectError(errMsg(e));
      setReconnectStatus("error");
    }
  };

  const renderStatus = (
    status: StepStatus,
    error: string,
    successKey: string,
    failKey: string,
  ) => {
    if (status === "success") {
      return (
        <span className="text-xs text-success flex items-center gap-1 select-none">
          <CheckCircle className="h-3 w-3 shrink-0" />
          {t(successKey)}
        </span>
      );
    }
    if (status === "error") {
      return (
        <span className="text-xs text-danger flex items-center gap-1 select-none break-all">
          <AlertCircle className="h-3 w-3 shrink-0" />
          {t(failKey)}{error ? `: ${error}` : ""}
        </span>
      );
    }
    return null;
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("browser-forensics.install-helper.title")}</DialogTitle>
          <DialogDescription>{t("browser-forensics.install-helper.dev-mode-hint")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2 max-h-[60vh] overflow-y-auto">
          {/* Step 1: 选择浏览器 */}
          <div className="space-y-1.5">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-browser")}</p>
            <div className="flex items-center gap-2">
              {BROWSERS.map((b) => (
                <Button
                  key={b.kind}
                  variant={browser === b.kind ? "default" : "secondary"}
                  size="sm"
                  onClick={() => setBrowser(b.kind)}
                >
                  {b.label}
                </Button>
              ))}
            </div>
          </div>

          {/* Step 2: 安装 NMH */}
          <div className="space-y-1.5">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-nmh")}</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Button
                variant="default"
                size="sm"
                onClick={handleInstallNmh}
                disabled={nmhStatus === "loading"}
              >
                <Package className="h-3 w-3" />
                {t("browser-forensics.install-helper.step-nmh-btn")}
              </Button>
              {renderStatus(
                nmhStatus,
                nmhError,
                "browser-forensics.install-helper.step-nmh-success",
                "browser-forensics.install-helper.step-nmh-fail",
              )}
            </div>
          </div>

          {/* Step 3: 复制扩展目录路径 */}
          <div className="space-y-1.5">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-path")}</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Button
                variant="default"
                size="sm"
                onClick={handleCopyPath}
                disabled={pathStatus === "loading"}
              >
                <Copy className="h-3 w-3" />
                {t("browser-forensics.install-helper.step-path-btn")}
              </Button>
              {renderStatus(
                pathStatus,
                pathError,
                "browser-forensics.install-helper.step-path-success",
                "browser-forensics.install-helper.step-path-fail",
              )}
            </div>
          </div>

          {/* Step 4: 打开浏览器扩展页 */}
          <div className="space-y-1.5">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-open")}</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Button
                variant="default"
                size="sm"
                onClick={handleOpenExtensionsPage}
                disabled={openPageStatus === "loading"}
              >
                <ExternalLink className="h-3 w-3" />
                {t("browser-forensics.install-helper.step-open-btn")}
              </Button>
              {renderStatus(
                openPageStatus,
                openPageError,
                "browser-forensics.install-helper.step-open-success",
                "browser-forensics.install-helper.step-open-fail",
              )}
            </div>
          </div>

          {/* Step 5: 加载已解压的扩展程序 */}
          <div className="space-y-1.5">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-load")}</p>
            <p className="text-xs text-fg-secondary select-none">{t("browser-forensics.install-helper.step-load-desc")}</p>
          </div>

          {/* Step 6: 重新连接（扩展不连接时使用） */}
          <div className="space-y-1.5 pt-2 border-t">
            <p className="text-sm font-medium select-none">{t("browser-forensics.install-helper.step-reconnect")}</p>
            <p className="text-xs text-fg-secondary select-none">{t("browser-forensics.install-helper.step-reconnect-desc")}</p>
            <div className="flex items-center gap-2 flex-wrap">
              <Button
                variant="default"
                size="sm"
                onClick={handleReconnect}
                disabled={reconnectStatus === "loading"}
              >
                <RefreshCw className="h-3 w-3" />
                {t("browser-forensics.install-helper.step-reconnect-btn")}
              </Button>
              {renderStatus(
                reconnectStatus,
                reconnectError,
                "browser-forensics.install-helper.step-reconnect-success",
                "browser-forensics.install-helper.step-reconnect-fail",
              )}
            </div>
            {/* 诊断信息 */}
            {diagnostics && (
              <div className="text-xs space-y-1 p-2 bg-muted/30 rounded select-none">
                <div className={diagnostics.nmh_exe_exists ? "text-success" : "text-danger"}>
                  NMH exe: {diagnostics.nmh_exe_exists ? "✓" : "✗"} {diagnostics.nmh_exe_path}
                </div>
                <div>
                  {t("browser-forensics.install-helper.diag-killed", { count: diagnostics.killed_processes, defaultValue: `killed ${diagnostics.killed_processes} processes` })}
                </div>
                <div className={diagnostics.connection.connected ? "text-success" : "text-muted-foreground"}>
                  {diagnostics.connection.connected
                    ? t("browser-forensics.connection.connected")
                    : t("browser-forensics.connection.disconnected")}
                </div>
                {!diagnostics.nmh_exe_exists && (
                  <div className="text-danger">
                    {t("browser-forensics.install-helper.diag-build-hint", { defaultValue: "Run: cargo build -p irtool-native-messaging" })}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* 高级选项（折叠）：兜底扩展 ID 输入 */}
          <div className="pt-2 border-t">
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="text-xs text-fg-secondary hover:text-fg select-none flex items-center gap-1"
            >
              <span>{showAdvanced ? "▼" : "▶"}</span>
              {t("browser-forensics.install-helper.advanced-toggle")}
            </button>
            {showAdvanced && (
              <div className="mt-2 space-y-1.5">
                <p className="text-xs text-fg-secondary select-none">
                  {t("browser-forensics.install-helper.advanced-extid-desc")}
                </p>
                <input
                  type="text"
                  value={extensionIdOverride}
                  onChange={(e) => setExtensionIdOverride(e.target.value.trim())}
                  placeholder="abcdefghijklmnopabcdefghijklmnop"
                  className="w-full px-2 py-1 text-xs font-mono border rounded bg-background select-none"
                  spellCheck={false}
                  autoComplete="off"
                />
                <p className="text-xs text-muted-foreground select-none">
                  {t("browser-forensics.install-helper.advanced-extid-hint")}
                </p>
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
