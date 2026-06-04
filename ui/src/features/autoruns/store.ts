import { create } from "zustand";
import type { ScanProgress, SignatureProgress } from "./types";

export interface AutorunsFilters {
  search: string;
  category: string;
  status: "all" | "enabled" | "disabled";
  signature: "all" | "valid" | "invalid" | "unsigned";
}

type DetailPosition = "bottom" | "right";

interface AutorunsState {
  filters: AutorunsFilters;
  setFilter: <K extends keyof AutorunsFilters>(key: K, value: AutorunsFilters[K]) => void;
  resetFilters: () => void;

  detailPosition: DetailPosition;
  setDetailPosition: (pos: DetailPosition) => void;

  selectedEntryId: number | null;
  setSelectedEntryId: (id: number | null) => void;

  scanProgress: ScanProgress | null;
  setScanProgress: (p: ScanProgress | null) => void;

  signatureProgress: SignatureProgress | null;
  setSignatureProgress: (p: SignatureProgress | null) => void;

  scanning: boolean;
  setScanning: (v: boolean) => void;

  verifyingSignatures: boolean;
  setVerifyingSignatures: (v: boolean) => void;

  calculatingHash: boolean;
  setCalculatingHash: (v: boolean) => void;

  hashProgress: SignatureProgress | null;
  setHashProgress: (p: SignatureProgress | null) => void;

  error: string | null;
  setError: (e: string | null) => void;

  lastScanDuration: number | null;
  setLastScanDuration: (d: number | null) => void;

  sigcheckResult: { path: string; output: string } | null;
  setSigcheckResult: (r: { path: string; output: string } | null) => void;
}

const DEFAULT_FILTERS: AutorunsFilters = {
  search: "",
  category: "all",
  status: "all",
  signature: "all",
};

export const useAutorunsStore = create<AutorunsState>((set) => ({
  filters: DEFAULT_FILTERS,
  setFilter: (key, value) => set((s) => ({ filters: { ...s.filters, [key]: value } })),
  resetFilters: () => set({ filters: DEFAULT_FILTERS }),

  detailPosition: "bottom",
  setDetailPosition: (detailPosition) => set({ detailPosition }),

  selectedEntryId: null,
  setSelectedEntryId: (selectedEntryId) => set({ selectedEntryId }),

  scanProgress: null,
  setScanProgress: (scanProgress) => set({ scanProgress }),

  signatureProgress: null,
  setSignatureProgress: (signatureProgress) => set({ signatureProgress }),

  scanning: false,
  setScanning: (scanning) => set({ scanning }),

  verifyingSignatures: false,
  setVerifyingSignatures: (verifyingSignatures) => set({ verifyingSignatures }),

  calculatingHash: false,
  setCalculatingHash: (calculatingHash) => set({ calculatingHash }),

  hashProgress: null,
  setHashProgress: (hashProgress) => set({ hashProgress }),

  error: null,
  setError: (error) => set({ error }),

  lastScanDuration: null,
  setLastScanDuration: (lastScanDuration) => set({ lastScanDuration }),

  sigcheckResult: null,
  setSigcheckResult: (sigcheckResult) => set({ sigcheckResult }),
}));
