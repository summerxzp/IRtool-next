import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnOrderState,
  type ColumnSizingState,
  type SortingState,
  type RowSelectionState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";

export interface DataTableProps<T> {
  columns: ColumnDef<T, unknown>[];
  data: T[];
  rowHeight?: number;
  onRowSelect?: (row: T | null) => void;
  getRowId: (row: T) => string;
  rowClassName?: (row: T) => string | undefined;
  onRowContextMenu?: (row: T, event: React.MouseEvent) => void;
  persistKey?: string;
  empty?: React.ReactNode;
  density?: "compact" | "normal";
  selectedRowId?: string | null;
}

export function DataTable<T>({
  columns,
  data,
  rowHeight,
  onRowSelect,
  getRowId,
  rowClassName,
  onRowContextMenu,
  persistKey,
  empty,
  density = "compact",
  selectedRowId,
}: DataTableProps<T>) {
  const storagePrefix = persistKey ? `datatable-${persistKey}` : null;

  const [sorting, setSorting] = React.useState<SortingState>([]);
  const [rowSelection, setRowSelection] = React.useState<RowSelectionState>({});
  const tableContainerRef = React.useRef<HTMLDivElement>(null);

  // Column order with persistence
  const [columnOrder, setColumnOrder] = React.useState<ColumnOrderState>(() => {
    if (!storagePrefix) return [];
    try {
      const saved = localStorage.getItem(`${storagePrefix}-col-order`);
      return saved ? JSON.parse(saved) : [];
    } catch { return []; }
  });

  // Column sizing with persistence
  const [columnSizing, setColumnSizing] = React.useState<ColumnSizingState>(() => {
    if (!storagePrefix) return {};
    try {
      const saved = localStorage.getItem(`${storagePrefix}-col-sizing`);
      return saved ? JSON.parse(saved) : {};
    } catch { return {}; }
  });

  const persistColumnOrder = React.useCallback((order: ColumnOrderState) => {
    if (!storagePrefix) return;
    try { localStorage.setItem(`${storagePrefix}-col-order`, JSON.stringify(order)); } catch {}
  }, [storagePrefix]);

  const persistColumnSizing = React.useCallback((sizing: ColumnSizingState) => {
    if (!storagePrefix) return;
    try { localStorage.setItem(`${storagePrefix}-col-sizing`, JSON.stringify(sizing)); } catch {}
  }, [storagePrefix]);

  const computedRowHeight = rowHeight ?? (density === "compact" ? 28 : 34);

  const table = useReactTable({
    data,
    columns,
    state: { sorting, rowSelection, columnOrder, columnSizing },
    onSortingChange: setSorting,
    onRowSelectionChange: setRowSelection,
    onColumnOrderChange: (updater) => {
      setColumnOrder((old) => {
        const next = typeof updater === "function" ? updater(old) : updater;
        persistColumnOrder(next);
        return next;
      });
    },
    onColumnSizingChange: (updater) => {
      setColumnSizing((old) => {
        const next = typeof updater === "function" ? updater(old) : updater;
        persistColumnSizing(next);
        return next;
      });
    },
    enableMultiRowSelection: false,
    enableColumnResizing: true,
    columnResizeMode: "onChange",
    getRowId,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  // Track whether selection change is from external sync (not user action)
  const isExternalSyncRef = React.useRef(false);

  // Sync external selectedRowId to internal rowSelection
  React.useEffect(() => {
    if (selectedRowId === undefined) return; // uncontrolled
    const currentId = Object.keys(rowSelection)[0] ?? null;
    if (currentId === selectedRowId) return;
    isExternalSyncRef.current = true;
    if (selectedRowId == null) {
      setRowSelection({});
    } else {
      setRowSelection({ [selectedRowId]: true });
    }
  }, [selectedRowId]);

  // Use ref for onRowSelect to avoid effect re-firing on every render
  const onRowSelectRef = React.useRef(onRowSelect);
  onRowSelectRef.current = onRowSelect;

  React.useEffect(() => {
    // Skip onRowSelect callback for external syncs to avoid infinite loops
    if (isExternalSyncRef.current) {
      isExternalSyncRef.current = false;
      return;
    }
    const cb = onRowSelectRef.current;
    if (!cb) return;
    const ids = Object.keys(rowSelection);
    if (ids.length === 0) {
      cb(null);
    } else {
      const row = data.find((d) => getRowId(d) === ids[0]);
      cb(row ?? null);
    }
  }, [rowSelection, data, getRowId]);

  const rows = table.getRowModel().rows;

  // Keyboard navigation
  React.useEffect(() => {
    const container = tableContainerRef.current;
    if (!container) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      if (rows.length === 0) return;

      const selectedIds = Object.keys(rowSelection);
      const currentIdx = selectedIds.length > 0
        ? rows.findIndex((r) => r.id === selectedIds[0])
        : -1;

      let nextIdx: number;
      if (e.key === "ArrowDown") {
        nextIdx = currentIdx < rows.length - 1 ? currentIdx + 1 : currentIdx;
      } else {
        nextIdx = currentIdx > 0 ? currentIdx - 1 : 0;
      }

      if (nextIdx !== currentIdx) {
        e.preventDefault();
        rows[nextIdx].toggleSelected(true);
        const rowEl = container.querySelector(`[data-row-idx="${nextIdx}"]`);
        rowEl?.scrollIntoView({ block: "nearest" });
      }
    };

    container.addEventListener("keydown", handleKeyDown);
    return () => container.removeEventListener("keydown", handleKeyDown);
  }, [rows, rowSelection]);

  // Column drag-and-drop
  const [draggedColumn, setDraggedColumn] = React.useState<string | null>(null);
  const [dragOverColumn, setDragOverColumn] = React.useState<string | null>(null);

  const handleDrop = (e: React.DragEvent, targetColumnId: string) => {
    e.preventDefault();
    if (!draggedColumn || draggedColumn === targetColumnId) {
      setDraggedColumn(null);
      setDragOverColumn(null);
      return;
    }

    const currentOrder = table.getState().columnOrder.length > 0
      ? table.getState().columnOrder
      : table.getVisibleLeafColumns().map((c) => c.id);

    const srcIdx = currentOrder.indexOf(draggedColumn);
    const dstIdx = currentOrder.indexOf(targetColumnId);
    if (srcIdx === -1 || dstIdx === -1) return;

    const newOrder = [...currentOrder];
    newOrder.splice(srcIdx, 1);
    newOrder.splice(dstIdx, 0, draggedColumn);
    setColumnOrder(newOrder);
    persistColumnOrder(newOrder);

    setDraggedColumn(null);
    setDragOverColumn(null);
  };

  const handleDragEnd = () => {
    setDraggedColumn(null);
    setDragOverColumn(null);
  };

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => computedRowHeight,
    overscan: 12,
  });

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();
  const paddingTop = virtualItems[0]?.start ?? 0;
  const paddingBottom =
    virtualItems.length > 0
      ? totalSize - (virtualItems[virtualItems.length - 1].end ?? 0)
      : 0;

  return (
    <div
      ref={tableContainerRef}
      className="h-full w-full overflow-auto bg-bg-base"
      tabIndex={0}
    >
      <table
        className="w-full text-sm font-sans border-collapse"
        style={{ width: table.getCenterTotalSize() }}
      >
        <thead className="sticky top-0 z-10 bg-bg-elev-1">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b border-border">
              {hg.headers.map((header) => {
                const size = header.getSize();
                return (
                  <th
                    key={header.id}
                    className={cn(
                      "h-7 px-2 text-left font-medium text-fg-secondary text-xs select-none relative group",
                      header.column.getCanSort() && "cursor-pointer hover:text-fg-primary",
                      draggedColumn === header.id && "opacity-50",
                      dragOverColumn === header.id && "border-l-2 border-accent",
                    )}
                    style={{ width: size, minWidth: 40 }}
                    onClick={header.column.getToggleSortingHandler()}
                    draggable
                    onDragStart={(e) => {
                      setDraggedColumn(header.id);
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", header.id);
                    }}
                    onDragOver={(e) => {
                      e.preventDefault();
                      e.dataTransfer.dropEffect = "move";
                      setDragOverColumn(header.id);
                    }}
                    onDragLeave={() => setDragOverColumn(null)}
                    onDrop={(e) => handleDrop(e, header.id)}
                    onDragEnd={handleDragEnd}
                  >
                    <div className="flex items-center gap-1">
                      {flexRender(header.column.columnDef.header, header.getContext())}
                      {{
                        asc: <span className="text-accent">▲</span>,
                        desc: <span className="text-accent">▼</span>,
                      }[header.column.getIsSorted() as string] ?? null}
                    </div>
                    {/* Column resize handle */}
                    {header.column.getCanResize() && (
                      <div
                        className="absolute right-0 top-0 h-full w-1.5 cursor-col-resize hover:bg-accent/50 active:bg-accent"
                        onMouseDown={header.getResizeHandler()}
                        onClick={(e) => e.stopPropagation()}
                      />
                    )}
                  </th>
                );
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                colSpan={columns.length}
                className="text-center text-fg-tertiary py-8"
              >
                {empty ?? "暂无数据"}
              </td>
            </tr>
          ) : (
            <>
              {paddingTop > 0 && (
                <tr style={{ height: paddingTop }}>
                  <td colSpan={columns.length} />
                </tr>
              )}
              {virtualItems.map((vRow) => {
                const row = rows[vRow.index];
                const original = row.original as T;
                const isSelected = row.getIsSelected();
                return (
                  <tr
                    key={row.id}
                    data-row-idx={vRow.index}
                    className={cn(
                      "border-b border-border/50 cursor-pointer transition-colors",
                      isSelected
                        ? "bg-bg-elev-2"
                        : "hover:bg-bg-elev-2/40",
                      rowClassName?.(original),
                    )}
                    style={{ height: computedRowHeight }}
                    onClick={() => row.toggleSelected()}
                    onContextMenu={(e) => {
                      if (!isSelected) row.toggleSelected();
                      onRowContextMenu?.(original, e);
                    }}
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td
                        key={cell.id}
                        className="px-2 truncate text-fg-primary text-sm"
                        style={{ width: cell.column.getSize() }}
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                );
              })}
              {paddingBottom > 0 && (
                <tr style={{ height: paddingBottom }}>
                  <td colSpan={columns.length} />
                </tr>
              )}
            </>
          )}
        </tbody>
      </table>
    </div>
  );
}
