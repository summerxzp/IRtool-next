import { useEffect, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { SignatureBadge } from "./components/SignatureBadge";
import type { AutorunItem } from "./types";

// Shared in-memory icon cache: path -> data URL ("" means no icon)
const iconCache = new Map<string, string>();

export function clearIconCache() {
  iconCache.clear();
}

/** Batch preload icons into cache. Called after scan completes. */
export async function preloadIcons(items: AutorunItem[]) {
  const { batchExtractIcons } = await import("./api");
  const paths = items
    .map((i) => i.image_path)
    .filter((p): p is string => !!p);

  if (paths.length === 0) return;

  // Mark all as loading (empty string = no icon, undefined = not loaded yet)
  // We use a sentinel to distinguish "loading" from "no icon"
  for (const p of paths) {
    if (!iconCache.has(p)) iconCache.set(p, "");
  }

  try {
    const results = await batchExtractIcons(paths);
    for (const [path, icon] of results) {
      iconCache.set(path, icon ?? "");
    }
  } catch {
    // Silently fail - individual icons will be missing
  }
}

function EntryWithIcon({ entry, imagePath }: { entry: string; imagePath: string | null }) {
  const cached = imagePath ? iconCache.get(imagePath) : undefined;
  const [iconSrc, setIconSrc] = useState<string | null>(
    cached !== undefined ? cached : null
  );

  useEffect(() => {
    if (!imagePath) return;
    // Check cache
    const cached = iconCache.get(imagePath);
    if (cached !== undefined) {
      setIconSrc(cached);
      return;
    }
    // Not in cache - shouldn't happen after batch preload, but handle gracefully
    setIconSrc(null);
  }, [imagePath]);

  return (
    <span className="flex items-center gap-1.5">
      {iconSrc ? (
        <img src={iconSrc} alt="" className="w-4 h-4 shrink-0" />
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
