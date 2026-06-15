import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import * as api from "./api";
import { useNetworkStore } from "./store";
import type { CmdlineStatus, NetworkEnrichmentPayload, NetworkSnapshotPayload } from "./types";

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

  // Listen for cmdline enrichment events and patch cached data
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<NetworkEnrichmentPayload>("evt_network_enrichment", (e) => {
      const { pid, cmdline_status, process_cmdline } = e.payload;
      console.log("[Network] Received enrichment event:", pid, cmdline_status, process_cmdline?.slice(0, 50));
      qc.setQueryData<NetworkSnapshotPayload>(QK_NETWORK, (old) => {
        if (!old) {
          console.log("[Network] No cached data to update");
          return old;
        }
        const updated = {
          ...old,
          items: old.items.map((conn) =>
            conn.pid === pid
              ? { ...conn, cmdline_status: cmdline_status as CmdlineStatus, process_cmdline: process_cmdline ?? conn.process_cmdline }
              : conn
          ),
        };
        console.log("[Network] Updated", updated.items.filter(c => c.pid === pid).length, "connections for pid", pid);
        return updated;
      });
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [qc]);

  useEffect(() => {
    const id = setTimeout(() => {
      api
        .setPolling({
          interval_ms: intervalMs,
          paused,
          retention,
        })
        .catch(console.error);
    }, 300);
    return () => clearTimeout(id);
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
      qc.setQueryData(QK_NETWORK, { items: [], timestamp: 0 });
      qc.invalidateQueries({ queryKey: QK_NETWORK });
    },
  });
}
