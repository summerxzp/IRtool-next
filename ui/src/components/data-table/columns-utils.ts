import type { ColumnDef } from "@tanstack/react-table";

export function lookupColumnSize<T>(
  columns: ColumnDef<T, unknown>[],
  id: string,
  fallback = 100,
): number {
  const col = columns.find((c) => (c as any).id === id || (c as any).accessorKey === id);
  return (col as any)?.size ?? fallback;
}

export function persistColumnSizes(key: string, sizes: Record<string, number>) {
  try {
    localStorage.setItem(`irtool-cols-${key}`, JSON.stringify(sizes));
  } catch {
    /* ignore */
  }
}

export function loadColumnSizes(key: string): Record<string, number> {
  try {
    const raw = localStorage.getItem(`irtool-cols-${key}`);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}
