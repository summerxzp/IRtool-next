// Re-export from bindings — these types will be auto-generated
export interface ProcessEntry {
  pid: number;
  ppid: number;
  name: string;
  exe: string | null;
  is_suspicious: boolean;
  suspicious_reason: string | null;
}

export interface ProcessSnapshot {
  processes: ProcessEntry[];
  timestamp: number;
}

export interface ProcessNode {
  pid: number;
  name: string;
  exe: string | null;
  cmdline: string | null;
  create_time: string | null;
  is_target: boolean;
  is_suspicious: boolean;
  suspicious_reason: string | null;
}

export interface ProcessChain {
  nodes: ProcessNode[];
}

export type ViewMode = "list" | "tree";
export type FilterMode = "all" | "suspicious";

export interface ProcessTreeNode extends ProcessEntry {
  children: ProcessTreeNode[];
  isOrphan: boolean;
}
