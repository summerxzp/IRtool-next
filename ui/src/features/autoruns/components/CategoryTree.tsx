import { useMemo } from "react";
import { cn } from "@/lib/utils";
import type { AutorunItem } from "../types";

interface Props {
  data: AutorunItem[];
  selectedCategory: string;
  onSelectCategory: (category: string) => void;
}

export function CategoryTree({ data, selectedCategory, onSelectCategory }: Props) {
  const categories = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of data) {
      map.set(item.category, (map.get(item.category) ?? 0) + 1);
    }
    return Array.from(map.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [data]);

  const total = data.length;

  return (
    <div className="h-full overflow-auto py-1 text-xs">
      <button
        className={cn(
          "w-full text-left px-3 py-1.5 hover:bg-bg-elev-2/40 transition-colors whitespace-nowrap",
          selectedCategory === "all" && "bg-bg-elev-2 text-accent"
        )}
        onClick={() => onSelectCategory("all")}
      >
        <span className="font-medium">全部</span>
        <span className="ml-2 text-fg-tertiary">{total}</span>
      </button>
      {categories.map(([cat, count]) => (
        <button
          key={cat}
          className={cn(
            "w-full text-left px-3 py-1.5 hover:bg-bg-elev-2/40 transition-colors whitespace-nowrap",
            selectedCategory === cat && "bg-bg-elev-2 text-accent"
          )}
          onClick={() => onSelectCategory(cat)}
        >
          <span className="font-medium">{cat}</span>
          <span className="ml-2 text-fg-tertiary">{count}</span>
        </button>
      ))}
    </div>
  );
}
