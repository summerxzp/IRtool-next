import { useCallback, useSyncExternalStore } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { ProcessEntry } from "./types";

// ── Reactive icon cache ──────────────────────────────────────────
const MAX_ICON_CACHE = 500;
export const iconCache = new Map<string, string>();
const pathListeners = new Map<string, Set<() => void>>();

export function subscribePath(path: string, listener: () => void): () => void {
  if (!pathListeners.has(path)) {
    pathListeners.set(path, new Set());
  }
  pathListeners.get(path)!.add(listener);
  return () => {
    const set = pathListeners.get(path);
    if (set) {
      set.delete(listener);
      if (set.size === 0) pathListeners.delete(path);
    }
  };
}

function notifyPath(path: string) {
  pathListeners.get(path)?.forEach((l) => l());
}

function trimIconCache() {
  if (iconCache.size > MAX_ICON_CACHE) {
    const excess = iconCache.size - MAX_ICON_CACHE;
    let count = 0;
    for (const key of iconCache.keys()) {
      if (count >= excess) break;
      iconCache.delete(key);
      count++;
    }
  }
}

/** Batch preload icons into cache. Skips paths already cached. */
export async function preloadIcons(paths: string[]) {
  const { batchExtractIcons } = await import("./api");
  const uncachedPaths = paths.filter((p) => !!p && !iconCache.has(p));

  if (uncachedPaths.length === 0) return;

  try {
    const results = await batchExtractIcons(uncachedPaths);
    for (const [path, icon] of results) {
      iconCache.set(path, icon ?? "");
      notifyPath(path);
    }
    trimIconCache();
  } catch {
    // Silently fail - individual icons will be missing
  }
}

// ── EntryWithIcon component ──────────────────────────────────────

export function EntryWithIcon({ name, imagePath }: { name: string; imagePath: string | null }) {
  const subscribe = useCallback(
    (listener: () => void) => {
      if (!imagePath) return () => {};
      return subscribePath(imagePath, listener);
    },
    [imagePath]
  );

  const iconSrc = useSyncExternalStore(
    subscribe,
    () => {
      if (!imagePath) return null;
      const cached = iconCache.get(imagePath);
      return cached && cached !== "" ? cached : null;
    },
    () => null
  );

  return (
    <span className="flex items-center gap-1.5">
      {iconSrc ? (
        <img src={iconSrc} alt="" className="w-4 h-4 shrink-0 object-contain" />
      ) : (
        <span className="w-4 h-4 shrink-0 inline-block rounded-sm bg-bg-elev-2" />
      )}
      <span className="font-medium text-fg-primary truncate">{name}</span>
    </span>
  );
}

// ── Column definitions ───────────────────────────────────────────

export const processColumns: ColumnDef<ProcessEntry>[] = [
  {
    id: "pid",
    accessorFn: (r) => r.pid,
    header: "PID",
    size: 70,
    cell: ({ row }) => <span className="font-mono text-xs">{row.original.pid}</span>,
  },
  {
    id: "ppid",
    accessorFn: (r) => r.ppid,
    header: "PPID",
    size: 70,
    cell: ({ row }) => <span className="font-mono text-xs">{row.original.ppid}</span>,
  },
  {
    id: "name",
    accessorFn: (r) => r.name,
    header: "名称",
    size: 160,
    cell: ({ row }) => (
      <EntryWithIcon
        name={row.original.name}
        imagePath={row.original.exe}
      />
    ),
  },
  {
    id: "exe",
    accessorFn: (r) => r.exe ?? "",
    header: "路径",
    size: 350,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary truncate">{row.original.exe ?? ""}</span>
    ),
  },
  {
    id: "suspicious",
    accessorFn: (r) => r.is_suspicious,
    header: "可疑",
    size: 80,
    cell: ({ row }) =>
      row.original.is_suspicious ? (
        <span className="text-warning text-xs" title={row.original.suspicious_reason ?? undefined}>⚠</span>
      ) : (
        <span className="text-fg-tertiary text-xs">—</span>
      ),
  },
];
