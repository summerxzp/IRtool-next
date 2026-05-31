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
    <div style={{ padding: 24, fontFamily: "system-ui, sans-serif" }}>
      <h1>IRtool v2 boot</h1>
      {info && (
        <pre>{JSON.stringify(info, null, 2)}</pre>
      )}
      {error && <p style={{ color: "red" }}>error: {error}</p>}
    </div>
  );
}

export default App;
