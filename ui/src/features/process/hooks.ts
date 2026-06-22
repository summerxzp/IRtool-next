import { useQuery } from "@tanstack/react-query";
import * as api from "./api";

const QK_PROCESS_SNAPSHOT = ["process", "snapshot"] as const;

export function useProcessSnapshot() {
  return useQuery({
    queryKey: QK_PROCESS_SNAPSHOT,
    queryFn: api.getSnapshot,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });
}

export function useProcessChain(pid: number | null) {
  return useQuery({
    queryKey: ["process", "chain", pid],
    queryFn: () => api.getProcessChain(pid!),
    enabled: pid != null,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });
}

export function useNetworkByPid(pid: number | null) {
  return useQuery({
    queryKey: ["process", "network", pid],
    queryFn: () => api.getNetworkByPid(pid!),
    enabled: pid != null,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });
}

export function useAutorunsByPath(exePath: string | null) {
  return useQuery({
    queryKey: ["process", "autoruns", exePath],
    queryFn: () => api.getAutorunsByPath(exePath!),
    enabled: exePath != null,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });
}
