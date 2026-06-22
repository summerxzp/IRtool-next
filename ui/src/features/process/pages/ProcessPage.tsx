import { useMemo, useEffect, useRef } from "react";
import { Panel, Group, Separator } from "react-resizable-panels";
import { Route } from "@/routes/process";
import { useProcessSnapshot } from "../hooks";
import { useProcessStore } from "../store";
import { useUIStore } from "@/stores/ui-store";
import { ProcessToolbar } from "../components/ProcessToolbar";
import { ProcessTable } from "../components/ProcessTable";
import { ProcessTreeView } from "../components/ProcessTreeView";
import { ProcessDetail } from "../components/ProcessDetail";
import { preloadIcons } from "../columns";
import type { ProcessEntry } from "../types";

function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function ProcessPage() {
  const routeSearch = Route.useSearch();
  const query = useProcessSnapshot();
  const selectedPid = useProcessStore((s) => s.selectedPid);
  const setSelectedPid = useProcessStore((s) => s.setSelectedPid);
  const viewMode = useProcessStore((s) => s.viewMode);
  const filter = useProcessStore((s) => s.filter);
  const search = useProcessStore((s) => s.search);
  const expandAll = useProcessStore((s) => s.expandAll);
  const autoRefreshMs = useProcessStore((s) => s.autoRefreshMs);
  const detailPosition = useUIStore((s) => s.detailPositions["process"] ?? "right");

  const data = useMemo(() => query.data?.processes ?? [], [query.data]);
  const snapshotTime = useMemo(() => formatTimestamp(query.data?.timestamp ?? 0), [query.data]);

  // Auto-refresh timer
  const refreshRef = useRef(query.refetch);
  refreshRef.current = query.refetch;
  useEffect(() => {
    if (autoRefreshMs <= 0) return;
    const id = setInterval(() => refreshRef.current(), autoRefreshMs);
    return () => clearInterval(id);
  }, [autoRefreshMs]);

  useEffect(() => {
    if (data.length === 0) return;
    const uniquePaths = [...new Set(data.map((p) => p.exe).filter((p): p is string => !!p))];
    preloadIcons(uniquePaths);
  }, [data]);

  // Auto-select process when navigating with search params
  useEffect(() => {
    if (routeSearch.pid) {
      setSelectedPid(routeSearch.pid);
    } else if (routeSearch.imagePath && data.length > 0) {
      const match = data.find(p => p.exe === routeSearch.imagePath);
      if (match) {
        setSelectedPid(match.pid);
      }
    }
  }, [routeSearch.pid, routeSearch.imagePath, data, setSelectedPid]);

  const filteredData = useMemo(() => {
    let result = data;
    if (filter === "suspicious") {
      result = result.filter((r) => r.is_suspicious);
    }
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter((r) =>
        r.name.toLowerCase().includes(q) ||
        (r.exe?.toLowerCase().includes(q) ?? false) ||
        String(r.pid).includes(q) ||
        (r.suspicious_reason?.toLowerCase().includes(q) ?? false)
      );
    }
    // Default sort: suspicious first, then by PID
    result = [...result].sort((a, b) => {
      if (a.is_suspicious !== b.is_suspicious) return a.is_suspicious ? -1 : 1;
      return a.pid - b.pid;
    });
    return result;
  }, [data, filter, search]);

  const selectedEntry = useMemo(() => data.find((d) => d.pid === selectedPid) ?? null, [data, selectedPid]);

  const handleRowSelect = (row: ProcessEntry | null) => {
    setSelectedPid(row?.pid ?? null);
  };

  const handleRefresh = () => {
    query.refetch();
  };

  return (
    <div className="h-full flex flex-col">
      <ProcessToolbar onRefresh={handleRefresh} loading={query.isLoading} snapshotTime={snapshotTime} />

      <div className="flex-1 min-h-0">
        {detailPosition === "bottom" ? (
          <Group orientation="vertical">
            <Panel defaultSize={60} minSize={30}>
              {viewMode === "list" ? (
                <ProcessTable data={filteredData} onRowSelect={handleRowSelect} selectedRowId={selectedPid != null ? String(selectedPid) : null} />
              ) : (
                <ProcessTreeView processes={filteredData} selectedPid={selectedPid} onSelect={setSelectedPid} expandAll={expandAll} />
              )}
            </Panel>
            {selectedPid != null && (
              <>
                <Separator className="h-px bg-border hover:bg-accent transition-colors" />
                <Panel defaultSize={40} minSize={20}>
                  <ProcessDetail entry={selectedEntry} snapshotTime={snapshotTime} onClose={() => setSelectedPid(null)} />
                </Panel>
              </>
            )}
          </Group>
        ) : (
          <Group orientation="horizontal">
            <Panel defaultSize={60} minSize={30}>
              {viewMode === "list" ? (
                <ProcessTable data={filteredData} onRowSelect={handleRowSelect} selectedRowId={selectedPid != null ? String(selectedPid) : null} />
              ) : (
                <ProcessTreeView processes={filteredData} selectedPid={selectedPid} onSelect={setSelectedPid} expandAll={expandAll} />
              )}
            </Panel>
            {selectedPid != null && (
              <>
                <Separator className="w-px bg-border hover:bg-accent transition-colors" />
                <Panel defaultSize={40} minSize={20}>
                  <ProcessDetail entry={selectedEntry} snapshotTime={snapshotTime} onClose={() => setSelectedPid(null)} />
                </Panel>
              </>
            )}
          </Group>
        )}
      </div>
    </div>
  );
}
