export type Proto = "tcp" | "udp";

export type Family = "v4" | "v6";

export type ConnState =
  | "CLOSED"
  | "LISTEN"
  | "SYN_SENT"
  | "SYN_RCVD"
  | "ESTABLISHED"
  | "FIN_WAIT1"
  | "FIN_WAIT2"
  | "CLOSE_WAIT"
  | "CLOSING"
  | "LAST_ACK"
  | "TIME_WAIT"
  | "DELETE_TCB"
  | "NONE";

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
  process_cmdline: string | null;
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
