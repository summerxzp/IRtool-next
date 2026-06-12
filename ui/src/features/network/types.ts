export type {
  NetConn,
  ConnState,
  Family,
  Proto,
  NetEndpoint,
  NetworkSnapshotPayload,
  RetentionPolicyDto,
  NetworkPollingControl,
  CmdlineStatus,
} from "@/lib/bindings";

export interface NetworkEnrichmentPayload {
  pid: number;
  cmdline_status: import("@/lib/bindings").CmdlineStatus;
  process_cmdline: string | null;
}
