import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
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
}

export function DataTable<T>({
  columns,
  data,
  rowHeight,
  onRowSelect,
  getRowId,
  rowClassName,
  onRowContextMenu,
  persistKey: _persistKey,
  empty,
  density = "compact",
}: DataTableProps<T>) {
  const [sorting, setSorting] = React.useState<SortingState>([]);
  const [rowSelection, setRowSelection] = React.useState<RowSelectionState>({});
  const tableContainerRef = React.useRef<HTMLDivElement>(null);

  const computedRowHeight = rowHeight ?? (density === "compact" ? 28 : 34);

  const table = useReactTable({
    data,
    columns,
    state: { sorting, rowSelection },
    onSortingChange: setSorting,
    onRowSelectionChange: setRowSelection,
    enableMultiRowSelection: false,
    getRowId,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  React.useEffect(() => {
    if (!onRowSelect) return;
    const ids = Object.keys(rowSelection);
    if (ids.length === 0) {
      onRowSelect(null);
    } else {
      const row = data.find((d) => getRowId(d) === ids[0]);
      onRowSelect(row ?? null);
    }
  }, [rowSelection, data, onRowSelect, getRowId]);

  const rows = table.getRowModel().rows;

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
    >
      <table className="w-full text-sm font-sans border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-elev-1">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b border-border">
              {hg.headers.map((header) => (
                <th
                  key={header.id}
                  className={cn(
                    "h-7 px-2 text-left font-medium text-fg-secondary text-xs select-none",
                    header.column.getCanSort() && "cursor-pointer hover:text-fg-primary",
                  )}
                  style={{ width: header.column.columnDef.size, minWidth: 60 }}
                  onClick={header.column.getToggleSortingHandler()}
                >
                  <div className="flex items-center gap-1">
                    {flexRender(header.column.columnDef.header, header.getContext())}
                    {{
                      asc: <span className="text-accent">▲</span>,
                      desc: <span className="text-accent">▼</span>,
                    }[header.column.getIsSorted() as string] ?? null}
                  </div>
                </th>
              ))}
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
                        style={{ width: cell.column.columnDef.size }}
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
