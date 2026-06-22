import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { tauriStorage } from "@/lib/tauri-storage";

export type DetailPosition = "right" | "bottom";

interface UIState {
  detailPositions: Record<string, DetailPosition>;
  setDetailPosition: (page: string, pos: DetailPosition) => void;
}

const DEFAULT_POSITIONS: Record<string, DetailPosition> = {
  autoruns: "right",
  "log-collector": "right",
  network: "right",
  workspace: "right",
  process: "right",
};

export const useUIStore = create<UIState>()(
  persist(
    (set) => ({
      detailPositions: { ...DEFAULT_POSITIONS },

      setDetailPosition: (page: string, pos: DetailPosition) => {
        set((s) => ({
          detailPositions: { ...s.detailPositions, [page]: pos },
        }));
      },
    }),
    {
      name: "irtool-ui",
      storage: createJSONStorage(() => tauriStorage),
      partialize: (state) => ({ detailPositions: state.detailPositions }),
    },
  ),
);
