import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./styles/globals.css";
import "./lib/i18n";
import { ThemeProvider } from "@/components/theme/ThemeProvider";
import { routeTree } from "./routeTree.gen";

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
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
