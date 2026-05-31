import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

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
    <div className="min-h-screen p-6 space-y-4">
      <h1 className="text-lg font-semibold">IRtool v2 boot</h1>
      <Separator />
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger">error: {error}</p>}
      <div className="flex gap-2">
        <Button variant="default">Primary</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="destructive">Destructive</Button>
      </div>
    </div>
  );
}

export default App;
