export type Proto = "tcp" | "udp";

export type Family = "ipv4" | "ipv6";

export type ConnState =
  | "ESTABLISHED"
  | "LISTEN"
  | "TIME_WAIT"
  | "CLOSE_WAIT"
  | "SYN_SENT"
  | "SYN_RCVD"
  | "None";

export interface NetEndpoint {
  addr: string;
  port: number;
}

export interface NetConn {
  proto: Proto;
  family: Family;
  local: NetEndpoint;
  remote: NetEndpoint;
  state: ConnState;
  pid: number;
  process_name: string | null;
  process_path: string | null;
  first_seen: number;
  last_seen: number;
  is_current: boolean;
}

export interface NetworkSnapshotPayload {
  connections: NetConn[];
  timestamp: number;
}

export type RetentionPolicyDto =
  | "Forever"
  | "None"
  | { Seconds: number };

export interface NetworkPollingControl {
  interval_ms: number;
  paused: boolean;
  retention: RetentionPolicyDto;
}
