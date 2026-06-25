import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useBrowserForensicsStore } from "../store";
import * as api from "../api";
import type { BrowserContext, MatchedExtension, CurrentTab, NavChainNode, RecentActivity } from "../types";
import { formatTimestamp } from "../utils";

function RiskBadge({ flags }: { flags: string[] }) {
  if (flags.length === 0) return null;
  const { t } = useTranslation();
  return (
    <span className="inline-flex gap-1 flex-wrap">
      {flags.map((f) => (
        <span
          key={f}
          className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-warning/15 text-warning"
        >
          {t(`browser-forensics.risk.${f.replace(/_/g, "-")}`, { defaultValue: f })}
        </span>
      ))}
    </span>
  );
}

function NavChainView({ nodes }: { nodes: NavChainNode[] }) {
  const { t } = useTranslation();
  if (nodes.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-navigation-chain")}</div>;
  }
  return (
    <div className="space-y-0.5">
      {nodes.map((node, i) => (
        <div key={i} className="flex items-start gap-2 text-xs">
          <span className="text-muted-foreground shrink-0 w-4 text-right">{i + 1}.</span>
          <div className="min-w-0 flex-1">
            <div className="text-fg-primary truncate" title={node.url}>{node.url}</div>
            <div className="flex gap-2 text-[10px] text-muted-foreground">
              {node.title && <span className="truncate">{node.title}</span>}
              {node.transition && (
                <span className="bg-bg-elev-2 px-1 rounded">{node.transition}</span>
              )}
            </div>
          </div>
          {i < nodes.length - 1 && (
            <span className="text-muted-foreground shrink-0">↓</span>
          )}
        </div>
      ))}
    </div>
  );
}

function RecentActivityView({ items }: { items: RecentActivity[] }) {
  const { t } = useTranslation();
  if (items.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-recent-activity")}</div>;
  }
  return (
    <div className="space-y-0.5 max-h-40 overflow-y-auto">
      {items.map((item, i) => (
        <div key={i} className="flex items-start gap-2 text-xs">
          <span className={`shrink-0 text-[10px] font-mono px-1 rounded ${
            item.tier === "immediate" ? "bg-danger/15 text-danger" :
            item.tier === "nearby" ? "bg-warning/15 text-warning" :
            "bg-info/15 text-info"
          }`}>
            {item.tier === "immediate" ? "IMM" : item.tier === "nearby" ? "NBY" : "REC"}
          </span>
          <div className="min-w-0 flex-1">
            <div className="text-fg-primary truncate" title={item.url}>{item.title || item.url}</div>
            <div className="text-[10px] text-muted-foreground">
              {item.time_distance_ms > 0 ? `+${item.time_distance_ms}ms` : `${item.time_distance_ms}ms`}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function MatchingExtensionsView({ extensions: exts }: { extensions: MatchedExtension[] }) {
  const { t } = useTranslation();
  if (exts.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-matching-extensions")}</div>;
  }
  return (
    <div className="space-y-1">
      {exts.map((ext) => (
        <div key={ext.id} className="flex items-start gap-2 p-1.5 rounded bg-bg-elev-1 text-xs">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="font-medium text-fg-primary truncate">{ext.name}</span>
              <span className="text-muted-foreground shrink-0">v{ext.version}</span>
            </div>
            <div className="text-[10px] text-muted-foreground font-mono truncate">{ext.id}</div>
            {ext.matched_patterns.length > 0 && (
              <div className="mt-0.5 text-[10px] text-code">
                Match: {ext.matched_patterns.join(", ")}
              </div>
            )}
            {ext.has_sensitive_permissions && (
              <div className="mt-0.5 text-[10px] text-danger">
                ⚠ Sensitive: webRequest + &lt;all_urls&gt;
              </div>
            )}
            <RiskBadge flags={ext.risk_flags} />
          </div>
        </div>
      ))}
    </div>
  );
}

function CurrentTabsView({ tabs }: { tabs: CurrentTab[] }) {
  const { t } = useTranslation();
  if (tabs.length === 0) {
    return <div className="text-xs text-muted-foreground italic">{t("browser-forensics.context.no-recovered-tabs")}</div>;
  }
  return (
    <div className="space-y-0.5 max-h-32 overflow-y-auto">
      {tabs.map((tab, i) => (
        <div key={i} className="flex items-center gap-2 text-xs">
          {tab.active && <span className="text-success shrink-0">●</span>}
          <div className="min-w-0 flex-1 truncate" title={tab.url}>{tab.title || tab.url}</div>
        </div>
      ))}
    </div>
  );
}

function ContextDetailView({ context }: { context: NonNullable<BrowserContext["context"]> }) {
  const { t } = useTranslation();
  return (
    <div className="grid grid-cols-2 gap-4 p-3">
      <div className="space-y-1.5">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {t("browser-forensics.context.recent-activity")}
        </h4>
        <RecentActivityView items={context.recent_browser_activity} />
      </div>
      <div className="space-y-1.5">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {t("browser-forensics.context.navigation-chain")}
        </h4>
        <NavChainView nodes={context.navigation_chain} />
      </div>
      <div className="space-y-1.5">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {t("browser-forensics.context.matching-extensions")}
        </h4>
        <MatchingExtensionsView extensions={context.matching_extensions} />
      </div>
      <div className="space-y-1.5">
        <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          {t("browser-forensics.context.current-tabs")}
        </h4>
        <CurrentTabsView tabs={context.current_tabs} />
      </div>
    </div>
  );
}

export function ContextAttributionPanel() {
  const { t } = useTranslation();

  const contextInputDomain = useBrowserForensicsStore((s) => s.contextInputDomain);
  const setContextInputDomain = useBrowserForensicsStore((s) => s.setContextInputDomain);
  const contextInputPid = useBrowserForensicsStore((s) => s.contextInputPid);
  const setContextInputPid = useBrowserForensicsStore((s) => s.setContextInputPid);
  const contextResult = useBrowserForensicsStore((s) => s.contextResult);
  const setContextResult = useBrowserForensicsStore((s) => s.setContextResult);
  const contextLoading = useBrowserForensicsStore((s) => s.contextLoading);
  const setContextLoading = useBrowserForensicsStore((s) => s.setContextLoading);

  const [processName, setProcessName] = useState("chrome.exe");
  const [cmdline, setCmdline] = useState("");

  const handleQuery = async () => {
    if (!contextInputDomain.trim()) return;
    setContextLoading(true);
    const result = await api.attributeBrowserContext(
      contextInputDomain.trim(),
      null,
      processName,
      parseInt(contextInputPid) || 0,
      cmdline.trim() || undefined,
    );
    setContextResult(result);
    setContextLoading(false);
  };

  // Summary card for the connection
  const connection = contextResult?.malicious_connection;
  const detail = contextResult?.context;

  return (
    <div className="h-full flex flex-col">
      {/* Input area */}
      <div className="px-3 py-2 border-b border-border flex items-center gap-2 flex-wrap">
        <Input
          className="w-48 h-8 text-sm"
          placeholder={t("browser-forensics.context.domain-placeholder")}
          value={contextInputDomain}
          onChange={(e) => setContextInputDomain(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleQuery()}
        />
        <Input
          className="w-28 h-8 text-sm"
          placeholder={t("browser-forensics.context.process-name")}
          value={processName}
          onChange={(e) => setProcessName(e.target.value)}
        />
        <Input
          className="w-48 h-8 text-sm"
          placeholder={t("browser-forensics.context.process-cmdline")}
          value={cmdline}
          onChange={(e) => setCmdline(e.target.value)}
        />
        <Input
          className="w-20 h-8 text-sm"
          placeholder={t("browser-forensics.context.pid")}
          value={contextInputPid}
          onChange={(e) => setContextInputPid(e.target.value)}
        />
        <Button size="sm" onClick={handleQuery} disabled={contextLoading}>
          {contextLoading ? t("browser-forensics.context.querying") : t("browser-forensics.context.query")}
        </Button>
        {connection && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setContextResult(null)}
          >
            {t("browser-forensics.context.clear")}
          </Button>
        )}
      </div>

      {/* Result area */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {!connection && (
          <div className="p-6 text-sm text-muted-foreground text-center flex flex-col items-center gap-2 pt-12">
            <span className="text-2xl">🔍</span>
            <p>
              {t("browser-forensics.context.domain-placeholder")}, {t("browser-forensics.context.process-name")}, {t("browser-forensics.context.process-cmdline")}, {t("browser-forensics.context.pid")}
            </p>
          </div>
        )}

        {connection && (
          <div className="p-3">
            {/* Connection summary card */}
            <div className="rounded-lg border border-border bg-bg-elev-1 p-3 mb-3">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                {t("browser-forensics.context.connection")}
              </h3>
              <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
                <div className="flex gap-2">
                  <span className="text-muted-foreground">Domain:</span>
                  <span className="text-fg-primary font-mono">{connection.domain}</span>
                </div>
                {connection.ip && (
                  <div className="flex gap-2">
                    <span className="text-muted-foreground">IP:</span>
                    <span className="text-fg-primary font-mono">{connection.ip}</span>
                  </div>
                )}
                <div className="flex gap-2">
                  <span className="text-muted-foreground">Process:</span>
                  <span className="text-fg-primary">{connection.process}</span>
                </div>
                <div className="flex gap-2">
                  <span className="text-muted-foreground">PID:</span>
                  <span className="text-fg-primary">{connection.pid}</span>
                </div>
                <div className="flex gap-2">
                  <span className="text-muted-foreground">Browser:</span>
                  <span className="text-fg-primary">{connection.browser}</span>
                </div>
                <div className="flex gap-2">
                  <span className="text-muted-foreground">Profile:</span>
                  <span className="text-fg-primary">{connection.profile || "(unknown)"}</span>
                </div>
                <div className="flex gap-2">
                  <span className="text-muted-foreground">Timestamp:</span>
                  <span className="text-fg-primary text-[10px]">{formatTimestamp(connection.timestamp)}</span>
                </div>
              </div>
            </div>

            {/* Context details */}
            {detail && <ContextDetailView context={detail} />}
          </div>
        )}
      </div>
    </div>
  );
}
