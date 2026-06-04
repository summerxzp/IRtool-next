import { useQuery, useQueryClient, useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import * as api from "./api";
import { useAutorunsStore } from "./store";
import type { ScanProgress, SignatureProgress } from "./types";

const QK_AUTORUNS = ["autoruns", "items"] as const;

export function useAutorunsData() {
  const qc = useQueryClient();
  const setScanProgress = useAutorunsStore((s) => s.setScanProgress);
  const setSignatureProgress = useAutorunsStore((s) => s.setSignatureProgress);
  const setScanning = useAutorunsStore((s) => s.setScanning);
  const setVerifyingSignatures = useAutorunsStore((s) => s.setVerifyingSignatures);
  const setError = useAutorunsStore((s) => s.setError);

  const query = useQuery({
    queryKey: QK_AUTORUNS,
    queryFn: api.getResult,
    retry: false,
    refetchOnWindowFocus: false,
  });

  const settersRef = useRef({ setScanProgress, setSignatureProgress, setScanning, setVerifyingSignatures, setError, setLastScanDuration: useAutorunsStore.getState().setLastScanDuration });
  settersRef.current = { setScanProgress, setSignatureProgress, setScanning, setVerifyingSignatures, setError, setLastScanDuration: useAutorunsStore.getState().setLastScanDuration };

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<ScanProgress>("evt_autoruns_progress", (e) => {
      const { setScanProgress, setScanning, setLastScanDuration } = settersRef.current;
      setScanProgress(e.payload);
      if (e.payload.phase === "complete") {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
        setScanning(false);
        // Extract duration from message like "扫描完成，共 287 项，耗时 12.3s"
        const match = e.payload.message.match(/耗时\s+([\d.]+)s/);
        if (match) setLastScanDuration(parseFloat(match[1]));
      }
    }).then((u) => { unlisten = u; });
    return () => { unlisten?.(); };
  }, [qc]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<SignatureProgress>("evt_autoruns_signature_progress", (e) => {
      const { setSignatureProgress, setVerifyingSignatures } = settersRef.current;
      setSignatureProgress(e.payload);
      // Refresh data periodically during verification (every 100 items)
      if (e.payload.current % 200 === 0) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
      }
      if (e.payload.current >= e.payload.total) {
        qc.invalidateQueries({ queryKey: QK_AUTORUNS });
        setVerifyingSignatures(false);
      }
    }).then((u) => { unlisten = u; });
    return () => { unlisten?.(); };
  }, [qc]);

  // Listen for task cancelled/failed events
  useEffect(() => {
    const unlistens: UnlistenFn[] = [];
    listen<{ task_id: number; error: unknown }>("evt_task_failed", (e) => {
      const { setScanning, setVerifyingSignatures, setError } = settersRef.current;
      setScanning(false);
      setVerifyingSignatures(false);
      const errObj = e.payload.error as Record<string, unknown>;
      const msg = errObj?.message ? String(errObj.message) : JSON.stringify(e.payload.error);
      setError(msg);
    }).then((u) => { unlistens.push(u); });

    listen<number>("evt_task_cancelled", () => {
      const { setScanning, setVerifyingSignatures } = settersRef.current;
      setScanning(false);
      setVerifyingSignatures(false);
    }).then((u) => { unlistens.push(u); });

    return () => { unlistens.forEach((u) => u()); };
  }, []);

  return query;
}

export function useAutorunsScan() {
  const setScanning = useAutorunsStore((s) => s.setScanning);
  const setError = useAutorunsStore((s) => s.setError);
  return useMutation({
    mutationFn: api.scan,
    onMutate: () => { setScanning(true); setError(null); },
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
  return useMutation({
    mutationFn: api.deleteEntry,
    onSuccess: () => { qc.invalidateQueries({ queryKey: QK_AUTORUNS }); },
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
