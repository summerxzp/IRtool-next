import { useTranslation } from "react-i18next";
import { X, Trash2, ExternalLink, FolderOpen, Terminal, Link } from "lucide-react";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useWorkspaceStore } from "../store";
import { useRunCommand } from "../hooks";
import * as api from "../api";
import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";
import type { WorkspaceTab, Rule } from "../types";
import { networkKey, eventKey } from "../rules/engine";
import { toast } from "sonner";

interface Props {
  onAssociation: () => void;
}

export function WorkspaceDetail({ onAssociation }: Props) {
  const { t } = useTranslation();
  const runCommandMutation = useRunCommand();

  const activeTab = useWorkspaceStore((s) => s.activeTab);
  const autorunItems = useWorkspaceStore((s) => s.autorunItems);
  const networkItems = useWorkspaceStore((s) => s.networkItems);
  const eventItems = useWorkspaceStore((s) => s.eventItems);
  const selectedAutorunId = useWorkspaceStore((s) => s.selectedAutorunId);
  const selectedNetworkKey = useWorkspaceStore((s) => s.selectedNetworkKey);
  const selectedEventKey = useWorkspaceStore((s) => s.selectedEventKey);
  const autorunMatchedRules = useWorkspaceStore((s) => s.autorunMatchedRules);
  const networkMatchedRules = useWorkspaceStore((s) => s.networkMatchedRules);
  const eventMatchedRules = useWorkspaceStore((s) => s.eventMatchedRules);
  const setSelectedAutorunId = useWorkspaceStore((s) => s.setSelectedAutorunId);
  const setSelectedNetworkKey = useWorkspaceStore((s) => s.setSelectedNetworkKey);
  const setSelectedEventKey = useWorkspaceStore((s) => s.setSelectedEventKey);

  const selectedItem = (() => {
    switch (activeTab) {
      case "autoruns":
        return selectedAutorunId != null ? autorunItems.find((a) => a.id === selectedAutorunId) ?? null : null;
      case "network":
        return selectedNetworkKey != null ? networkItems.find((n) => networkKey(n) === selectedNetworkKey) ?? null : null;
      case "events":
        return selectedEventKey != null ? eventItems.find((e) => eventKey(e) === selectedEventKey) ?? null : null;
    }
  })();

  const matchedRules: Rule[] = (() => {
    if (!selectedItem) return [];
    switch (activeTab) {
      case "autoruns":
        return autorunMatchedRules.get((selectedItem as AutorunItem).id) ?? [];
      case "network":
        return networkMatchedRules.get(selectedNetworkKey ?? "") ?? [];
      case "events":
        return eventMatchedRules.get(selectedEventKey ?? "") ?? [];
    }
  })();

  const handleClose = () => {
    switch (activeTab) {
      case "autoruns": setSelectedAutorunId(null); break;
      case "network": setSelectedNetworkKey(null); break;
      case "events": setSelectedEventKey(null); break;
    }
  };

  if (!selectedItem) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm p-6 text-center">
        {t("workspace.detail.select-row")}
      </div>
    );
  }

  const handleDelete = async () => {
    const item = selectedItem as AutorunItem;
    try {
      await api.deleteEntry(item.id);
      toast.success(t("workspace.detail.deleted"));
      handleClose();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleJumpRegistry = () => {
    const item = selectedItem as AutorunItem;
    api.openRegedit(item.location);
  };

  const handleOpenExplorer = (path: string | null) => {
    if (path) api.openExplorer(path);
  };

  const handleKill = async () => {
    const item = selectedItem as NetConn;
    try {
      await api.killProcess(item.pid);
      toast.success(t("workspace.detail.killed"));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleRunCommand = async (program: string, args: string, label: string) => {
    try {
      await runCommandMutation.mutateAsync({ program, args });
      toast.success(t("workspace.detail.command-success", { command: label }));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="h-full overflow-auto p-4 space-y-3">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <DetailHeader item={selectedItem} tab={activeTab} />
        </div>
        <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={handleClose}>
          <X className="h-4 w-4" />
        </button>
      </div>

      <Separator />

      <DetailInfo item={selectedItem} tab={activeTab} />

      {matchedRules.length > 0 && (
        <>
          <Separator />
          <div>
            <div className="text-xs text-fg-tertiary mb-1">{t("workspace.detail.matched-rules")}</div>
            <div className="flex flex-wrap gap-1">
              {matchedRules.map((r) => (
                <Badge key={r.id} variant={r.severity === "critical" || r.severity === "high" ? "danger" : "warning"}>
                  {r.name}
                </Badge>
              ))}
            </div>
          </div>
        </>
      )}

      <Separator />

      <DetailActions
        item={selectedItem}
        tab={activeTab}
        onDelete={handleDelete}
        onJumpRegistry={handleJumpRegistry}
        onOpenExplorer={handleOpenExplorer}
        onKill={handleKill}
        onRunCommand={handleRunCommand}
        onAssociation={onAssociation}
      />
    </div>
  );
}

function DetailHeader({ item, tab }: { item: AutorunItem | NetConn | SysmonEvent; tab: WorkspaceTab }) {
  switch (tab) {
    case "autoruns": {
      const a = item as AutorunItem;
      return (
        <>
          <div className="text-sm font-medium text-fg-primary">{a.entry}</div>
          <div className="flex items-center gap-1.5 mt-1 text-xs text-fg-tertiary flex-wrap">
            <span>{a.category}</span>
            {!a.enabled && (
              <>
                <span>·</span>
                <span className="text-fg-tertiary">已禁用</span>
              </>
            )}
          </div>
        </>
      );
    }
    case "network": {
      const n = item as NetConn;
      return (
        <>
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="info">{n.proto.toUpperCase()}</Badge>
            <Badge variant="outline">{n.family.toUpperCase()}</Badge>
            {n.state && n.state !== "NONE" && <Badge>{n.state}</Badge>}
          </div>
          <div className="text-sm font-mono text-fg-primary">
            {n.local.addr}:{n.local.port} → {n.remote.addr || "*"}:{n.remote.port || "*"}
          </div>
        </>
      );
    }
    case "events": {
      const e = item as SysmonEvent;
      return (
        <>
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="info">{e.event_type}</Badge>
          </div>
          <div className="text-sm font-mono text-fg-primary">
            {e.query_name || e.destination_ip || e.target_filename || "-"}
          </div>
        </>
      );
    }
  }
}

function DetailInfo({ item, tab }: { item: AutorunItem | NetConn | SysmonEvent; tab: WorkspaceTab }) {
  const { t } = useTranslation();

  switch (tab) {
    case "autoruns": {
      const a = item as AutorunItem;
      return (
        <div className="space-y-2 text-xs">
          <DetailRow label={t("workspace.detail.image-path")} value={a.image_path} mono />
          <DetailRow label={t("workspace.detail.launch-string")} value={a.launch_string} mono />
          <DetailRow label={t("workspace.detail.location")} value={a.location} mono />
          <DetailRow label={t("workspace.detail.publisher")} value={a.publisher} />
        </div>
      );
    }
    case "network": {
      const n = item as NetConn;
      return (
        <div className="space-y-2 text-xs">
          <DetailRow label={t("workspace.detail.local")} value={`${n.local.addr}:${n.local.port}`} mono />
          <DetailRow label={t("workspace.detail.remote")} value={`${n.remote.addr}:${n.remote.port}`} mono />
          <DetailRow label={t("workspace.detail.process")} value={n.process_name} />
          <DetailRow label={t("workspace.detail.path")} value={n.process_path} mono />
        </div>
      );
    }
    case "events": {
      const e = item as SysmonEvent;
      return (
        <div className="space-y-2 text-xs">
          <DetailRow label={t("workspace.detail.type")} value={e.event_type} />
          <DetailRow label={t("workspace.detail.time")} value={e.timestamp} mono />
          {e.query_name && <DetailRow label={t("workspace.detail.domain")} value={e.query_name} mono />}
          {e.destination_ip && <DetailRow label={t("workspace.detail.destination-ip")} value={e.destination_ip} mono />}
          <DetailRow label={t("workspace.detail.process")} value={e.process_name} />
          <DetailRow label={t("workspace.detail.path")} value={e.process_path} mono />
        </div>
      );
    }
  }
}

function DetailActions({
  item,
  tab,
  onDelete,
  onJumpRegistry,
  onOpenExplorer,
  onKill,
  onRunCommand,
  onAssociation,
}: {
  item: AutorunItem | NetConn | SysmonEvent;
  tab: WorkspaceTab;
  onDelete: () => void;
  onJumpRegistry: () => void;
  onOpenExplorer: (path: string | null) => void;
  onKill: () => void;
  onRunCommand: (program: string, args: string, label: string) => void;
  onAssociation: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-wrap gap-2">
      {tab === "autoruns" && (
        <>
          <Button variant="destructive" size="sm" onClick={onDelete}>
            <Trash2 className="h-3.5 w-3.5 mr-1" />
            {t("workspace.detail.delete")}
          </Button>
          {(item as AutorunItem).location.includes("HKLM") || (item as AutorunItem).location.includes("HKCU") ? (
            <Button variant="secondary" size="sm" onClick={onJumpRegistry}>
              <ExternalLink className="h-3.5 w-3.5 mr-1" />
              {t("workspace.detail.jump-registry")}
            </Button>
          ) : null}
          <Button variant="secondary" size="sm" onClick={() => onOpenExplorer((item as AutorunItem).image_path)} disabled={!(item as AutorunItem).image_path}>
            <FolderOpen className="h-3.5 w-3.5 mr-1" />
            {t("workspace.detail.open-explorer")}
          </Button>
          <MoreOperationsAutorun item={item as AutorunItem} onRunCommand={onRunCommand} />
        </>
      )}
      {tab === "network" && (
        <>
          <Button variant="destructive" size="sm" onClick={onKill}>
            <Trash2 className="h-3.5 w-3.5 mr-1" />
            {t("workspace.detail.kill")}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => onOpenExplorer((item as NetConn).process_path)} disabled={!(item as NetConn).process_path}>
            <FolderOpen className="h-3.5 w-3.5 mr-1" />
            {t("workspace.detail.open-explorer")}
          </Button>
        </>
      )}
      <Button variant="secondary" size="sm" onClick={onAssociation}>
        <Link className="h-3.5 w-3.5 mr-1" />
        {t("workspace.detail.association")}
      </Button>
    </div>
  );
}

function MoreOperationsAutorun({
  item,
  onRunCommand,
}: {
  item: AutorunItem;
  onRunCommand: (program: string, args: string, label: string) => void;
}) {
  const { t } = useTranslation();

  const commands = [
    {
      label: t("workspace.detail.remove-hidden"),
      program: "attrib",
      args: `-s -h "${item.image_path ?? ""}"`,
      disabled: !item.image_path,
    },
    {
      label: t("workspace.detail.take-ownership"),
      program: "takeown",
      args: `/f "${item.image_path ?? ""}"`,
      disabled: !item.image_path,
    },
    {
      label: t("workspace.detail.sample"),
      program: "7z",
      args: `a -p "sample.7z" "${item.image_path ?? ""}"`,
      disabled: !item.image_path,
    },
  ];

  return (
    <DropdownMenu>
      <TooltipProvider delayDuration={300}>
        <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button variant="secondary" size="sm">
              <Terminal className="h-3.5 w-3.5 mr-1" />
              {t("workspace.detail.more-operations")}
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>
          {commands.map((c) => !c.disabled && (
            <div key={c.label} className="font-mono text-[10px]">{c.program} {c.args}</div>
          ))}
        </TooltipContent>
        </Tooltip>
      </TooltipProvider>
      <DropdownMenuContent>
        {commands.map((c) => (
          <DropdownMenuItem
            key={c.label}
            disabled={c.disabled}
            onClick={() => onRunCommand(c.program, c.args, c.label)}
          >
            {c.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function DetailRow({ label, value, mono = false }: { label: string; value?: string | null; mono?: boolean }) {
  if (!value) return null;
  return (
    <div>
      <div className="text-fg-tertiary">{label}</div>
      <div className={mono ? "font-mono break-all" : "break-all"}>{value}</div>
    </div>
  );
}
