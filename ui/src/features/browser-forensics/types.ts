export type BrowserKind = "chrome" | "edge";

export interface BrowserProfile {
  browser: BrowserKind;
  name: string;
  display_name: string | null;
  path: string;
}

export interface IocMatch {
  ioc_type: string;
  value: string;
  severity: string;
}

export interface ExtensionInfo {
  id: string;
  name: string;
  version: string;
  description: string | null;
  enabled: boolean;
  install_time: string | null;
  install_source: string | null;
  update_url: string | null;
  was_installed_by_default: boolean | null;
  permissions: string[];
  host_permissions: string[];
  has_content_scripts: boolean;
  has_background: boolean;
  preferences_tampered: boolean;
  risk_flags: string[];
  path: string;
  ioc_matches: IocMatch[];
}

export interface ExtensionInventory {
  browser: BrowserKind;
  profile: string;
  extensions: ExtensionInfo[];
}

export type DangerType =
  | "NOT_DANGEROUS"
  | "DANGEROUS_URL"
  | "DANGEROUS_CONTENT"
  | "DANGEROUS_HOST"
  | "UNCOMMON_URL"
  | "POTENTIALLY_UNWANTED"
  | "ALLOWLISTED_BY_POLICY"
  | "UNKNOWN";

export interface DownloadInfo {
  filename: string;
  local_path: string;
  download_url: string;
  referrer: string | null;
  start_time: string | null;
  end_time: string | null;
  total_bytes: number | null;
  danger_type: DangerType;
  opened: boolean;
  interrupt_reason: string | null;
  evidence_type: string;
}

export interface RecoveredTab {
  url: string;
  title: string;
  active: boolean;
  tab_index: number | null;
}

export interface SessionRecoveryResult {
  browser: BrowserKind;
  profile: string;
  tabs: RecoveredTab[];
  parse_errors: string[];
}

export type TimeTier = "immediate" | "nearby" | "recent";

export interface RecentActivity {
  url: string;
  title: string;
  visit_time: string;
  tier: TimeTier;
  time_distance_ms: number;
  evidence_type: string;
}

export interface HistoryAttribution {
  browser: BrowserKind;
  profile: string;
  recent_browser_activity: RecentActivity[];
  navigation_chain: NavChainNode[];
}

export interface HistoryEntry {
  url: string;
  title: string;
  visit_time: string;
  visit_count: number;
}

export interface HistoryList {
  browser: BrowserKind;
  profile: string;
  entries: HistoryEntry[];
}

export interface NavChainNode {
  url: string;
  title: string | null;
  transition: string | null;
}

export interface BrowserContext {
  malicious_connection: MaliciousConnection;
  context: BrowserContextDetail;
}

export interface MaliciousConnection {
  domain: string;
  ip: string | null;
  process: string;
  pid: number;
  browser: BrowserKind;
  profile: string;
  timestamp: string;
}

export interface BrowserContextDetail {
  recent_browser_activity: RecentActivity[];
  navigation_chain: NavChainNode[];
  current_tabs: CurrentTab[];
  recent_downloads: DownloadInfo[];
  matching_extensions: MatchedExtension[];
}

export interface CurrentTab {
  url: string;
  title: string;
  active: boolean;
  evidence_type: string;
}

export interface MatchedExtension {
  id: string;
  name: string;
  version: string;
  risk_flags: string[];
  matched_patterns: string[];
  has_sensitive_permissions: boolean;
}

export interface ExtensionAttribution {
  label: string;
  browser: BrowserKind;
  profile: string;
  pid: number;
  domain: string;
  candidate_extensions: MatchedExtension[];
}

/// 后端推送的浏览器恶意连接事件负载
export interface BrowserMaliciousConnectionPayload {
  domain: string;
  ip: string;
  process_name: string;
  pid: number;
  cmdline: string | null;
  alert_id: string;
}

/// 后端推送的扩展归因网络请求事件负载
export interface ExtensionAttributionPayload {
  timestamp: number;
  request_id: string;
  url: string;
  method: string;
  initiator: string | null;
  attribution_status: string;
  extension_id: string | null;
  extension_name: string | null;
}

/// 基于域名的归因结果
export interface DomainAttribution {
  target: string;
  browser: BrowserKind;
  profile: string;
  matching_extensions: MatchedExtension[];
  related_history: HistoryEntry[];
  related_downloads: DownloadInfo[];
  related_tabs: CurrentTab[];
}
