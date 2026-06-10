import { load, type Store } from "@tauri-apps/plugin-store";

let _store: Store | null = null;

async function getStore(): Promise<Store> {
  if (!_store) {
    _store = await load("settings.json", { autoSave: 100, defaults: {} });
  }
  return _store;
}

/**
 * A Storage-compatible adapter backed by tauri-plugin-store.
 * Data is persisted to config/settings.json instead of browser localStorage.
 *
 * getItem/setItem are async — zustand's createJSONStorage supports this.
 */
export const tauriStorage = {
  async getItem(key: string): Promise<string | null> {
    try {
      const store = await getStore();
      const val = await store.get<unknown>(key);
      if (val === null || val === undefined) return null;
      return typeof val === "string" ? val : JSON.stringify(val);
    } catch {
      return null;
    }
  },
  async setItem(key: string, value: string): Promise<void> {
    try {
      const store = await getStore();
      await store.set(key, value);
    } catch {
      // Silently fail — settings persistence is non-critical
    }
  },
  async removeItem(key: string): Promise<void> {
    try {
      const store = await getStore();
      await store.delete(key);
    } catch {
      // Silently fail
    }
  },
};
