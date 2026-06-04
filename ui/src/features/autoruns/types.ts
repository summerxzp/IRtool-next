// Re-export from bindings — these types will be auto-generated
// For now, define them manually to match the Rust types
export type RiskLevel = "safe" | "suspicious" | "high_risk";

export type SignatureStatus =
  | { kind: "valid"; detail: { signer: string } }
  | { kind: "invalid"; detail: { message: string } }
  | { kind: "unsigned" }
  | { kind: "not_verified" };

export interface AutorunItem {
  id: number;
  category: string;
  entry: string;
  enabled: boolean;
  location: string;
  description: string;
  publisher: string;
  image_path: string | null;
  launch_string: string | null;
  timestamp: string | null;
  file_exists: boolean;
  file_size: number | null;
  file_version: string | null;
  service_name: string | null;
  md5: string | null;
  sha256: string | null;
  risk: RiskLevel;
  risk_reasons: string[];
  signature: SignatureStatus;
}

export interface ScanOptions {
  include_hash: boolean;
  category_filter: string[] | null;
}

export type ScanPhase = "running_autorunsc" | "parsing_csv" | "checking_files" | "evaluating_risk" | "verifying_signatures" | "complete";

export interface ScanProgress {
  task_id: number;
  phase: ScanPhase;
  current: number;
  total: number;
  message: string;
}

export interface SignatureProgress {
  task_id: number;
  current: number;
  total: number;
}

export interface DeleteResult {
  success: boolean;
  message: string;
}
