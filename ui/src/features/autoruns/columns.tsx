import { useCallback, useSyncExternalStore } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { SignatureBadge } from "./components/SignatureBadge";
import type { AutorunItem } from "./types";

// ── Reactive icon cache ──────────────────────────────────────────
// Per-path listeners: when an icon for a specific path is loaded,
// only components subscribed to that path re-render.

const MAX_ICON_CACHE = 500;
const iconCache = new Map<string, string>();
const pathListeners = new Map<string, Set<() => void>>();

function subscribePath(path: string, listener: () => void): () => void {
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

export function clearIconCache() {
  iconCache.clear();
  for (const [, listeners] of pathListeners) {
    listeners.forEach((l) => l());
  }
}

/** Batch preload icons into cache. Skips paths already cached. */
export async function preloadIcons(items: AutorunItem[]) {
  const { batchExtractIcons } = await import("./api");
  const uncachedPaths = items
    .map((i) => i.image_path)
    .filter((p): p is string => !!p)
    .filter((p) => !iconCache.has(p));

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

function EntryWithIcon({ entry, imagePath }: { entry: string; imagePath: string | null }) {
  // Subscribe to cache changes for this specific path
  const subscribe = useCallback(
    (listener: () => void) => {
      if (!imagePath) return () => {};
      return subscribePath(imagePath, listener);
    },
    [imagePath]
  );

  // Read current icon from cache reactively
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
      <span className="font-medium text-fg-primary truncate">{entry}</span>
    </span>
  );
}

export const autorunsColumns: ColumnDef<AutorunItem>[] = [
  {
    id: "enabled",
    accessorFn: (r) => r.enabled,
    header: "启用",
    size: 50,
    cell: ({ row }) =>
      row.original.enabled ? (
        <span className="text-success text-xs">✓</span>
      ) : (
        <span className="text-fg-tertiary text-xs">✗</span>
      ),
  },
  {
    id: "category",
    accessorFn: (r) => r.category,
    header: "类别",
    size: 100,
  },
  {
    id: "signature",
    accessorFn: (r) => r.signature.kind,
    header: "签名",
    size: 80,
    cell: ({ row }) => <SignatureBadge status={row.original.signature} />,
  },
  {
    id: "entry",
    accessorFn: (r) => r.entry,
    header: "条目",
    size: 200,
    cell: ({ row }) => (
      <EntryWithIcon entry={row.original.entry} imagePath={row.original.image_path} />
    ),
  },
  {
    id: "image_path",
    accessorFn: (r) => r.image_path ?? "",
    header: "文件路径",
    size: 300,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-secondary">{row.original.image_path ?? ""}</span>
    ),
  },
  {
    id: "launch_string",
    accessorFn: (r) => r.launch_string ?? "",
    header: "启动命令",
    size: 260,
    cell: ({ row }) => (
      <span className="font-mono text-xs text-fg-tertiary truncate">{row.original.launch_string ?? ""}</span>
    ),
  },
  {
    id: "publisher",
    accessorFn: (r) => r.publisher,
    header: "发布者",
    size: 160,
  },
];
