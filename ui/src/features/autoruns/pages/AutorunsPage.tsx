import { useEffect, useMemo, useRef, useState } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { useTranslation } from "react-i18next";
import { Route } from "@/routes/autoruns";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { AlertDialog, AlertDialogContent, AlertDialogHeader, AlertDialogTitle, AlertDialogDescription, AlertDialogFooter, AlertDialogAction, AlertDialogCancel } from "@/components/ui/alert-dialog";
import { useAutorunsData, useAutorunsScan, useDeleteEntry, useCalculateHash, useSyncScanningState } from "../hooks";
import * as api from "../api";
import { useAutorunsStore } from "../store";
import { useUIStore } from "@/stores/ui-store";
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
  const search = Route.useSearch();
  const query = useAutorunsData();
  const scanMutation = useAutorunsScan();
  const deleteMutation = useDeleteEntry();
  const calculateHashMutation = useCalculateHash();

  // Sync scanning state with backend on mount (handles page navigation)
  useSyncScanningState();

  const selectedEntryId = useAutorunsStore((s) => s.selectedEntryId);
  const setSelectedEntryId = useAutorunsStore((s) => s.setSelectedEntryId);
  const scanning = useAutorunsStore((s) => s.scanning);
  const verifyingSignatures = useAutorunsStore((s) => s.verifyingSignatures);
  const signatureProgress = useAutorunsStore((s) => s.signatureProgress);
  const detailPosition = useUIStore((s) => s.detailPositions["autoruns"] ?? "right");
  const scanProgress = useAutorunsStore((s) => s.scanProgress);
  const error = useAutorunsStore((s) => s.error);
  const setError = useAutorunsStore((s) => s.setError);
  const success = useAutorunsStore((s) => s.success);
  const setSuccess = useAutorunsStore((s) => s.setSuccess);
  const setSigcheckResult = useAutorunsStore((s) => s.setSigcheckResult);
  const calculatingHash = useAutorunsStore((s) => s.calculatingHash);
  const setCalculatingHash = useAutorunsStore((s) => s.setCalculatingHash);
  const hashProgress = useAutorunsStore((s) => s.hashProgress);
  const filters = useAutorunsStore((s) => s.filters);

  useEffect(() => {
    if (success) {
      const timer = setTimeout(() => setSuccess(null), 4000);
      return () => clearTimeout(timer);
    }
  }, [success, setSuccess]);

  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<AutorunItem | null>(null);
  const [contextRow, setContextRow] = useState<AutorunItem | null>(null);
  const [contextPos, setContextPos] = useState<{ x: number; y: number } | null>(null);
  const [batchHashDialogOpen, setBatchHashDialogOpen] = useState(false);

  const data = useMemo(() => query.data ?? [], [query.data]);
  const selectedItem = useMemo(() => data.find((d) => d.id === selectedEntryId) ?? null, [data, selectedEntryId]);
  const categories = useMemo(() => { const set = new Set(data.map((d) => d.category)); return Array.from(set).sort(); }, [data]);

  // Auto-select item when navigating with imagePath search param
  useEffect(() => {
    if (search.imagePath && data.length > 0) {
      const match = data.find(d => d.image_path === search.imagePath);
      if (match) {
        setSelectedEntryId(match.id);
      }
    }
  }, [search.imagePath, data, setSelectedEntryId]);

  // Filtering logic (moved from AutorunsTable)
  const searchIndexRef = useRef(new Map<number, string>());
  // Clear index when data changes
  useMemo(() => { searchIndexRef.current.clear(); }, [data]);

  const filteredData = useMemo(() => {
    let result = data;
    if (filters.status === "enabled") {
      result = result.filter((r) => r.enabled);
    } else if (filters.status === "disabled") {
      result = result.filter((r) => !r.enabled);
    }
    if (filters.signature !== "all") {
      result = result.filter((r) => r.signature.kind === filters.signature);
    }
    if (filters.categories.length > 0) {
      result = result.filter((r) => filters.categories.includes(r.category));
    }
    if (filters.search.trim()) {
      const q = filters.search.toLowerCase();
      result = result.filter((r) => {
        let blob = searchIndexRef.current.get(r.id);
        if (blob === undefined) {
          blob = `${r.entry} ${r.image_path ?? ""} ${r.launch_string ?? ""} ${r.publisher} ${r.location} ${r.category}`.toLowerCase();
          searchIndexRef.current.set(r.id, blob);
        }
        return blob.includes(q);
      });
    }
    return result;
  }, [data, filters]);

  const handleScan = () => { scanMutation.mutate({ include_hash: false, category_filter: null }); };
  const handleCancel = () => { if (scanProgress) { import("../api").then((api) => api.cancelScan(scanProgress.task_id)); } };
  const handleExport = async () => {
    await exportCsv(
      data.map((d) => ({ category: d.category, entry: d.entry, enabled: d.enabled, location: d.location, image_path: d.image_path ?? "", publisher: d.publisher, risk: d.risk, signature: d.signature.kind })),
      ["category", "entry", "enabled", "location", "image_path", "publisher", "risk", "signature"],
      `irtool-autoruns-${Date.now()}.csv`
    );
  };
  const handleDelete = (item: AutorunItem) => { setDeleteTarget(item); setDeleteDialogOpen(true); };
  const handleConfirmDelete = (entryId: number) => {
    deleteMutation.mutate(entryId);
    // Close detail panel if the deleted item was selected
    if (selectedEntryId === entryId) {
      setSelectedEntryId(null);
    }
  };
  const handleJumpToRegistry = (item: AutorunItem) => { api.openRegedit(item.location); };
  const handleContextMenu = (row: AutorunItem, event: React.MouseEvent) => { event.preventDefault(); setContextRow(row); setContextPos({ x: event.clientX, y: event.clientY }); };

  const handleBatchCalculateHash = () => {
    setBatchHashDialogOpen(false);
    const entryIds = data.filter((d) => d.image_path && d.file_exists && !d.sha256).map((d) => d.id);
    if (entryIds.length > 0) {
      setCalculatingHash(true);
      api.batchCalculateHash(entryIds).then(() => {
        setCalculatingHash(false);
        query.refetch();
      }).catch((e) => {
        setCalculatingHash(false);
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
      });
    }
  };

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
      <AutorunsToolbar onScan={handleScan} onCancel={handleCancel} onBatchCalculateHash={() => setBatchHashDialogOpen(true)} onExport={handleExport} scanning={scanning} calculatingHash={calculatingHash} hasData={data.length > 0} categories={categories} />

      {error && (
        <div className="px-3 py-1.5 bg-danger/10 text-danger text-xs flex items-center gap-2">
          <span className="flex-1">{error}</span>
          <button className="text-danger/60 hover:text-danger" onClick={() => setError(null)}>✕</button>
        </div>
      )}

      {success && (
        <div className="px-3 py-1.5 bg-success/10 text-success text-xs flex items-center gap-2">
          <span className="flex-1">{success}</span>
          <button className="text-success/60 hover:text-success" onClick={() => setSuccess(null)}>✕</button>
        </div>
      )}

      <div className="flex-1 min-h-0">
        {detailPosition === "bottom" ? (
          <Group orientation="vertical">
            <Panel defaultSize={60} minSize={30}>
              <AutorunsTable data={filteredData} onRowSelect={(row) => setSelectedEntryId(row?.id ?? null)} onRowContextMenu={handleContextMenu} selectedRowId={selectedEntryId != null ? String(selectedEntryId) : null} />
            </Panel>
            {selectedEntryId != null && (
              <>
                <Separator className="h-px bg-border hover:bg-accent transition-colors" />
                <Panel defaultSize={40} minSize={20}>
                  <AutorunsDetail item={selectedItem} onDelete={handleDelete} onJumpToRegistry={handleJumpToRegistry} onSearchInWorkspace={() => {}} onClose={() => setSelectedEntryId(null)} />
                </Panel>
              </>
            )}
          </Group>
        ) : (
          <Group orientation="horizontal">
            <Panel defaultSize={60} minSize={30}>
              <AutorunsTable data={filteredData} onRowSelect={(row) => setSelectedEntryId(row?.id ?? null)} onRowContextMenu={handleContextMenu} selectedRowId={selectedEntryId != null ? String(selectedEntryId) : null} />
            </Panel>
            {selectedEntryId != null && (
              <>
                <Separator className="w-px bg-border hover:bg-accent transition-colors" />
                <Panel defaultSize={40} minSize={20}>
                  <AutorunsDetail item={selectedItem} onDelete={handleDelete} onJumpToRegistry={handleJumpToRegistry} onSearchInWorkspace={() => {}} onClose={() => setSelectedEntryId(null)} />
                </Panel>
              </>
            )}
          </Group>
        )}
      </div>

      <AutorunsStatsBar data={data} filteredCount={filteredData.length} scanning={scanning} scanProgress={scanProgress} verifyingSignatures={verifyingSignatures} signatureProgress={signatureProgress} calculatingHash={calculatingHash} hashProgress={hashProgress} />

      <DeleteEntryDialog item={deleteTarget} open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen} onConfirm={handleConfirmDelete} />

      <AlertDialog open={batchHashDialogOpen} onOpenChange={setBatchHashDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("autoruns.batch-hash.title")}</AlertDialogTitle>
            <AlertDialogDescription>{t("autoruns.batch-hash.message")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={handleBatchCalculateHash}>{t("common.confirm")}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

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
