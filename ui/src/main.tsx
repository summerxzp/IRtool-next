import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./styles/globals.css";
import "./lib/i18n";
import { ThemeProvider } from "@/components/theme/ThemeProvider";
import { Toaster } from "sonner";
import { routeTree } from "./routeTree.gen";

// Log unhandled JS errors to Rust backend
window.addEventListener("error", (e) => {
  console.error("[unhandled]", e.error ?? e.message);
  import("@/lib/bindings").then(({ commands }) => {
    commands.cmdLogFrontend(`ERROR: ${e.error?.stack ?? e.message}`);
  }).catch(() => {});
});
window.addEventListener("unhandledrejection", (e) => {
  console.error("[unhandled rejection]", e.reason);
  import("@/lib/bindings").then(({ commands }) => {
    commands.cmdLogFrontend(`REJECTION: ${e.reason?.stack ?? e.reason}`);
  }).catch(() => {});
});

const router = createRouter({ routeTree });

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 0, refetchOnWindowFocus: false, retry: 1 },
  },
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <RouterProvider router={router} />
        <Toaster />
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
