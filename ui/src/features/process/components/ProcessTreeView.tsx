import { useState, useMemo, useCallback, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, ChevronDown } from "lucide-react";
import { iconCache, subscribePath } from "../columns";
import type { ProcessEntry, ProcessTreeNode } from "../types";

interface Props {
  processes: ProcessEntry[];
  selectedPid: number | null;
  onSelect: (pid: number) => void;
}

function buildProcessTree(processes: ProcessEntry[]): ProcessTreeNode[] {
  const map = new Map<number, ProcessTreeNode>();
  const roots: ProcessTreeNode[] = [];

  for (const p of processes) {
    map.set(p.pid, { ...p, children: [], isOrphan: false });
  }

  for (const p of processes) {
    const node = map.get(p.pid)!;
    const parent = map.get(p.ppid);
    if (parent) {
      parent.children.push(node);
    } else {
      node.isOrphan = p.ppid !== 0 && p.ppid !== 4;
      roots.push(node);
    }
  }

  return roots;
}

function TreeNodeIcon({ imagePath }: { imagePath: string | null }) {
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

  if (iconSrc) {
    return <img src={iconSrc} alt="" className="w-4 h-4 shrink-0" />;
  }
  return <span className="w-4 h-4 shrink-0 inline-block rounded-sm bg-bg-elev-2" />;
}

function TreeNode({ node, depth, selectedPid, onSelect }: { node: ProcessTreeNode; depth: number; selectedPid: number | null; onSelect: (pid: number) => void }) {
  const [expanded, setExpanded] = useState(depth < 2 || node.is_suspicious);
  const hasChildren = node.children.length > 0;
  const isSelected = node.pid === selectedPid;

  return (
    <div>
      <div
        className={`flex items-center gap-1 px-2 cursor-pointer hover:bg-bg-elev-2/40 transition-colors ${
          isSelected ? "bg-bg-elev-2" : ""
        } ${node.isOrphan ? "border-l-2 border-warning" : ""} ${
          node.is_suspicious ? "text-warning" : "text-fg-primary"
        }`}
        style={{ paddingLeft: `${depth * 16 + 8}px`, height: 28 }}
        onClick={() => {
          onSelect(node.pid);
          if (hasChildren) setExpanded(!expanded);
        }}
      >
        {hasChildren ? (
          expanded ? (
            <ChevronDown className="h-3.5 w-3.5 shrink-0 text-fg-tertiary" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 shrink-0 text-fg-tertiary" />
          )
        ) : (
          <span className="w-3.5 shrink-0" />
        )}
        <TreeNodeIcon imagePath={node.exe} />
        <span className="font-mono text-xs text-fg-tertiary shrink-0">{node.pid}</span>
        <span className={`text-sm truncate select-none ${node.is_suspicious ? "font-medium" : ""}`}>
          {node.name}
        </span>
        {node.is_suspicious && <span className="text-warning text-xs ml-1" title={node.suspicious_reason ?? undefined}>⚠</span>}
      </div>
      {expanded && hasChildren && (
        <div>
          {node.children.map((child) => (
            <TreeNode key={child.pid} node={child} depth={depth + 1} selectedPid={selectedPid} onSelect={onSelect} />
          ))}
        </div>
      )}
    </div>
  );
}

export function ProcessTreeView({ processes, selectedPid, onSelect }: Props) {
  const { t } = useTranslation();
  const tree = useMemo(() => buildProcessTree(processes), [processes]);

  if (processes.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm">
        {t("common.empty")}
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto bg-bg-base">
      {tree.map((node) => (
        <TreeNode key={node.pid} node={node} depth={0} selectedPid={selectedPid} onSelect={onSelect} />
      ))}
    </div>
  );
}
