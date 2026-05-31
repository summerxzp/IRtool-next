import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Sun, Moon, Monitor } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useThemeStore } from "@/stores/theme-store";

interface AppInfo {
  name: string;
  version: string;
  build: string;
  is_admin: boolean;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { theme, setTheme } = useThemeStore();

  useEffect(() => {
    invoke<AppInfo>("cmd_app_info")
      .then(setInfo)
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div className="min-h-screen p-6 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">IRtool v2 boot</h1>
        <div className="flex gap-1">
          <Button
            variant={theme === "dark" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("dark")}
            title="Dark"
          >
            <Moon className="h-4 w-4" />
          </Button>
          <Button
            variant={theme === "light" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("light")}
            title="Light"
          >
            <Sun className="h-4 w-4" />
          </Button>
          <Button
            variant={theme === "system" ? "default" : "ghost"}
            size="icon"
            onClick={() => setTheme("system")}
            title="System"
          >
            <Monitor className="h-4 w-4" />
          </Button>
        </div>
      </div>
      <Separator />
      {info && (
        <pre className="bg-bg-elev-1 border border-border p-3 rounded font-mono text-sm text-fg-secondary">
          {JSON.stringify(info, null, 2)}
        </pre>
      )}
      {error && <p className="text-danger">error: {error}</p>}
    </div>
  );
}

export default App;
