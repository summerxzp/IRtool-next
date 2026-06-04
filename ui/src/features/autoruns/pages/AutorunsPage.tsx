import { useMemo, useState } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useAutorunsData, useAutorunsScan, useVerifySignatures, useDeleteEntry, useCalculateHash } from "../hooks";
import * as api from "../api";
import { useAutorunsStore } from "../store";
import { AutorunsToolbar } from "../components/AutorunsToolbar";
import { AutorunsTable } from "../components/AutorunsTable";
import { AutorunsDetail } from "../components/AutorunsDetail";
import { AutorunsStatsBar } from "../components/AutorunsStatsBar";
import { DeleteEntryDialog } from "../components/DeleteEntryDialog";
import { SigcheckDialog } from "../components/SigcheckDialog";
import { exportCsv } from "@/lib/csv";
import type { AutorunItem } from "../types";

export function AutorunsPage() {
  const { t } = useTranslation();
  const query = useAutorunsData();
  const scanMutation = useAutorunsScan();
  const verifyMutation = useVerifySignatures();
  const deleteMutation = useDeleteEntry();
  const calculateHashMutation = useCalculateHash();

  const selectedEntryId = useAutorunsStore((s) => s.selectedEntryId);
  const setSelectedEntryId = useAutorunsStore((s) => s.setSelectedEntryId);
  const scanning = useAutorunsStore((s) => s.scanning);
  const verifyingSignatures = useAutorunsStore((s) => s.verifyingSignatures);
  const signatureProgress = useAutorunsStore((s) => s.signatureProgress);
  const detailPosition = useAutorunsStore((s) => s.detailPosition);
  const scanProgress = useAutorunsStore((s) => s.scanProgress);
  const error = useAutorunsStore((s) => s.error);
  const setError = useAutorunsStore((s) => s.setError);
  const setSigcheckResult = useAutorunsStore((s) => s.setSigcheckResult);

  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AutorunItem | null>(null);
  const [contextRow, setContextRow] = useState<AutorunItem | null>(null);
  const [contextPos, setContextPos] = useState<{ x: number; y: number } | null>(null);

  const data = useMemo(() => query.data ?? [], [query.data]);
  const selectedItem = useMemo(() => data.find((d) => d.id === selectedEntryId) ?? null, [data, selectedEntryId]);
  const categories = useMemo(() => { const set = new Set(data.map((d) => d.category)); return Array.from(set).sort(); }, [data]);

  const handleScan = () => { scanMutation.mutate({ include_hash: false, category_filter: null }); };
  const handleCancel = () => { if (scanProgress) { import("../api").then((api) => api.cancelScan(scanProgress.task_id)); } };
  const handleVerifySignatures = () => {
    const paths = data.filter((d) => d.image_path && d.signature.kind === "not_verified" && d.file_exists).map((d) => d.image_path!);
    if (paths.length > 0) verifyMutation.mutate(paths);
  };
  const handleExport = async () => {
    await exportCsv(
      data.map((d) => ({ category: d.category, entry: d.entry, enabled: d.enabled, location: d.location, image_path: d.image_path ?? "", publisher: d.publisher, risk: d.risk, signature: d.signature.kind })),
      ["category", "entry", "enabled", "location", "image_path", "publisher", "risk", "signature"],
      `irtool-autoruns-${Date.now()}.csv`
    );
  };
  const handleDelete = (item: AutorunItem) => { setDeleteTarget(item); setDeleteDialogOpen(true); };
  const handleConfirmDelete = (entryId: number) => { deleteMutation.mutate(entryId); };
  const handleJumpToRegistry = (item: AutorunItem) => { api.openRegedit(item.location); };
  const handleContextMenu = (row: AutorunItem, event: React.MouseEvent) => { event.preventDefault(); setContextRow(row); setContextPos({ x: event.clientX, y: event.clientY }); };

  const handleContextCalculateHash = () => {
    if (!contextRow) return;
    calculateHashMutation.mutate(contextRow.id);
    setContextRow(null);
  };
  const handleContextSigcheck = async () => {
    if (!contextRow || !contextRow.image_path) return;
    try {
      const output = await api.sigcheck(contextRow.image_path);
      setSigcheckResult({ path: contextRow.image_path, output });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    }
    setContextRow(null);
  };
  const handleContextOpenExplorer = () => {
    if (!contextRow || !contextRow.image_path) return;
    api.openExplorer(contextRow.image_path);
    setContextRow(null);
  };
  const handleContextOpenServices = () => {
    api.openServices();
    setContextRow(null);
  };
  const handleContextOpenRegistry = () => {
    if (!contextRow) return;
    api.openRegedit(contextRow.location);
    setContextRow(null);
  };

  return (
    <div className="h-full flex flex-col">
      <AutorunsToolbar onScan={handleScan} onCancel={handleCancel} onVerifySignatures={handleVerifySignatures} onExport={handleExport} scanning={scanning} verifyingSignatures={verifyingSignatures} hasData={data.length > 0} categories={categories} />

      {error && (
        <div className="px-3 py-1.5 bg-danger/10 text-danger text-xs flex items-center gap-2">
          <span className="flex-1">{error}</span>
          <button className="text-danger/60 hover:text-danger" onClick={() => setError(null)}>✕</button>
        </div>
      )}

      <div className="flex-1 min-h-0">
        {detailPosition === "bottom" ? (
          <Group orientation="vertical">
            <Panel defaultSize={60} minSize={30}>
              <AutorunsTable data={data} onRowSelect={(row) => setSelectedEntryId(row?.id ?? null)} onRowContextMenu={handleContextMenu} />
            </Panel>
            <Separator className="h-px bg-border hover:bg-accent transition-colors" />
            <Panel defaultSize={40} minSize={20}>
              <AutorunsDetail item={selectedItem} onDelete={handleDelete} onJumpToRegistry={handleJumpToRegistry} onSearchInWorkspace={() => {}} />
            </Panel>
          </Group>
        ) : (
          <Group orientation="horizontal">
            <Panel defaultSize={60} minSize={30}>
              <AutorunsTable data={data} onRowSelect={(row) => setSelectedEntryId(row?.id ?? null)} onRowContextMenu={handleContextMenu} />
            </Panel>
            <Separator className="w-px bg-border hover:bg-accent transition-colors" />
            <Panel defaultSize={40} minSize={20}>
              <AutorunsDetail item={selectedItem} onDelete={handleDelete} onJumpToRegistry={handleJumpToRegistry} onSearchInWorkspace={() => {}} />
            </Panel>
          </Group>
        )}
      </div>

      <AutorunsStatsBar data={data} scanning={scanning} scanProgress={scanProgress} verifyingSignatures={verifyingSignatures} signatureProgress={signatureProgress} />

      <DeleteEntryDialog item={deleteTarget} open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen} onConfirm={handleConfirmDelete} />

      {contextRow && contextPos && (
        <DropdownMenu open={true} onOpenChange={() => setContextRow(null)}>
          <DropdownMenuTrigger asChild>
            <span className="fixed" style={{ top: contextPos.y, left: contextPos.x, width: 0, height: 0 }} />
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={() => { navigator.clipboard.writeText(`${contextRow.entry} ${contextRow.image_path ?? ""} ${contextRow.publisher}`); setContextRow(null); }}>
              {t("autoruns.context-menu.copy-row")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleContextCalculateHash}>
              {t("autoruns.context-menu.calculate-hash")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={handleContextSigcheck} disabled={!contextRow.image_path}>
              {t("autoruns.context-menu.sigcheck")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => { handleDelete(contextRow); setContextRow(null); }}>
              {t("autoruns.context-menu.delete")}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={handleContextOpenExplorer} disabled={!contextRow.image_path || !contextRow.file_exists}>
              {t("autoruns.context-menu.open-explorer")}
            </DropdownMenuItem>
            {contextRow.category === "Services" && (
              <DropdownMenuItem onClick={handleContextOpenServices}>
                {t("autoruns.context-menu.open-services")}
              </DropdownMenuItem>
            )}
            {(contextRow.location.includes("HKLM") || contextRow.location.includes("HKCU")) && (
              <DropdownMenuItem onClick={handleContextOpenRegistry}>
                {t("autoruns.context-menu.open-registry")}
              </DropdownMenuItem>
            )}
            {(contextRow.category === "Scheduled Tasks" || contextRow.category === "Tasks") && (
              <DropdownMenuItem disabled>
                {t("autoruns.detail.jump-task")}
              </DropdownMenuItem>
            )}
            <DropdownMenuSeparator />
            <DropdownMenuItem disabled onClick={() => setContextRow(null)}>
              {t("autoruns.context-menu.search-workspace")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      <SigcheckDialog />
    </div>
  );
}
