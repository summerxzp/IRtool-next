import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { StatusBar } from "./StatusBar";
import { setupAlertListener } from "@/stores/alert-store";
import { useSysmonEventListener, usePcapEventListener } from "@/features/log-collector/hooks";

setupAlertListener();

export function AppShell({ children }: { children: React.ReactNode }) {
  // Keep sysmon event listener alive across page navigation
  useSysmonEventListener();
  usePcapEventListener();

  return (
    <div className="flex flex-col h-screen bg-bg-base text-fg-primary">
      <TopBar />
      <div className="flex flex-1 min-h-0">
        <Sidebar />
        <div className="flex-1 flex flex-col min-w-0">
          <main className="flex-1 overflow-auto bg-bg-base">{children}</main>
          <StatusBar />
        </div>
      </div>
    </div>
  );
}
