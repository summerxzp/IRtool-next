import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface AppInfo {
  name: string;
  version: string;
  build: string;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="min-h-screen p-6">
      <h1 className="text-lg font-semibold text-fg-primary mb-4">
        IRtool v2 boot
      </h1>
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger mt-2">error: {error}</p>}
    </div>
  );
}

export default App;
