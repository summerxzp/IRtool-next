import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import * as api from "./api";
import { useAutorunsStore } from "./store";
import { preloadIcons } from "./columns";
import type { ScanProgress, SignatureProgress } from "./types";

const QK_AUTORUNS = ["autoruns", "items"] as const;

// Module-level flag to prevent duplicate listener registration
let listenersInitialized = false;

export function useAutorunsData() {
  const qc = useQueryClient();

  const query = useQuery({
    queryKey: QK_AUTORUNS,
    queryFn: api.getResult,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });

  useEffect(() => {
    if (listenersInitialized) return;
    listenersInitialized = true;

    // Autoruns scan progress - persistent listener (no cleanup on unmount)
    listen<ScanProgress>("evt_autoruns_progress", (e) => {
      const store = useAutorunsStore.getState();
      store.setScanProgress(e.payload);
      if (e.payload.phase === "complete") {
        api.getResult().then(async (newData) => {
          qc.setQueryData(QK_AUTORUNS, newData);
          if (newData && newData.length > 0) {
            setTimeout(() => preloadIcons(newData), 0);
          }
        });
        store.setScanning(false);
        const match = e.payload.message.match(/耗时\s+([\d.]+)s/);
        if (match) store.setLastScanDuration(parseFloat(match[1]));
      }
    });

    // Signature verification progress - persistent listener
    listen<SignatureProgress>("evt_autoruns_signature_progress", (e) => {
      const store = useAutorunsStore.getState();
      store.setSignatureProgress(e.payload);
      if (e.payload.current % 200 === 0) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
      }
      if (e.payload.current >= e.payload.total) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
        store.setVerifyingSignatures(false);
      }
    });

    // Hash progress - persistent listener
    listen<SignatureProgress>("evt_autoruns_hash_progress", (e) => {
      const store = useAutorunsStore.getState();
      store.setHashProgress(e.payload);
      if (e.payload.current % 200 === 0) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
      }
      if (e.payload.current >= e.payload.total) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
        store.setCalculatingHash(false);
        store.setHashProgress(null);
      }
    });

    // Task failed/cancelled - persistent listeners
    listen<{ task_id: number; error: unknown }>("evt_task_failed", (e) => {
      const store = useAutorunsStore.getState();
      store.setScanning(false);
      store.setVerifyingSignatures(false);
      const err = e.payload.error;
      const msg = typeof err === "string" ? err : (err instanceof Error ? err.message : JSON.stringify(err));
      store.setError(msg);
    });

    listen<number>("evt_task_cancelled", () => {
      const store = useAutorunsStore.getState();
      store.setScanning(false);
      store.setVerifyingSignatures(false);
    });
  }, [qc]);

  return query;
}

export function useAutorunsScan() {
  const setScanning = useAutorunsStore((s) => s.setScanning);
  const setError = useAutorunsStore((s) => s.setError);
  const setLastScanDuration = useAutorunsStore((s) => s.setLastScanDuration);
  return useMutation({
    mutationFn: api.scan,
    onMutate: () => { setScanning(true); setError(null); setLastScanDuration(null); },
    onError: (_err, _vars, _ctx) => {
      setScanning(false);
      const msg = _err instanceof Error ? _err.message : String(_err);
      setError(msg);
      console.error("[autoruns] scan failed:", msg);
    },
  });
}

export function useVerifySignatures() {
  const setVerifyingSignatures = useAutorunsStore((s) => s.setVerifyingSignatures);
  const setError = useAutorunsStore((s) => s.setError);
  return useMutation({
    mutationFn: api.verifySignatures,
    onMutate: () => { setVerifyingSignatures(true); setError(null); },
    onError: (_err) => {
      setVerifyingSignatures(false);
      const msg = _err instanceof Error ? _err.message : String(_err);
      setError(msg);
      console.error("[autoruns] verify failed:", msg);
    },
  });
}

export function useDeleteEntry() {
  const qc = useQueryClient();
  const setError = useAutorunsStore((s) => s.setError);
  return useMutation({
    mutationFn: api.deleteEntry,
    onSuccess: (result) => {
      qc.invalidateQueries({ queryKey: QK_AUTORUNS });
      if (!result.success) {
        setError(result.message);
      }
    },
    onError: (_err) => {
      const msg = _err instanceof Error ? _err.message : String(_err);
      setError(msg);
      console.error("[autoruns] delete failed:", msg);
    },
  });
}

export function useCalculateHash() {
  const qc = useQueryClient();
  const setError = useAutorunsStore((s) => s.setError);
  return useMutation({
    mutationFn: api.calculateHash,
    onSuccess: () => { qc.invalidateQueries({ queryKey: QK_AUTORUNS }); },
    onError: (_err) => {
      const msg = _err instanceof Error ? _err.message : String(_err);
      setError(msg);
    },
  });
}

/**
 * Sync frontend scanning state with backend on mount.
 * Same pattern as log-collector's useSyncCollectingState:
 * ask the backend for the truth, set it once.
 */
export function useSyncScanningState() {
  const setScanning = useAutorunsStore((s) => s.setScanning);

  useEffect(() => {
    (async () => {
      try {
        const backendScanning = await api.isScanning();
        setScanning(backendScanning);
      } catch {
        // ignore
      }
    })();
  }, [setScanning]);
}
