import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import * as api from "./api";
import { useNetworkStore } from "./store";
import type { NetworkSnapshotPayload } from "./types";

const QK_NETWORK = ["network", "snapshot"] as const;

export function useNetwork() {
  const qc = useQueryClient();
  const { paused, intervalMs, retention } = useNetworkStore();

  const query = useQuery({
    queryKey: QK_NETWORK,
    queryFn: api.snapshot,
  });

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<NetworkSnapshotPayload>("evt_network_snapshot", (e) => {
      qc.setQueryData(QK_NETWORK, e.payload);
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [qc]);

  useEffect(() => {
    api
      .setPolling({
        interval_ms: intervalMs,
        paused,
        retention,
      })
      .catch(console.error);
  }, [paused, intervalMs, retention]);

  return query;
}

export function useKillProcess() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: api.killProcess,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: QK_NETWORK });
    },
  });
}

export function useClearHistory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: api.clearHistory,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: QK_NETWORK });
    },
  });
}
