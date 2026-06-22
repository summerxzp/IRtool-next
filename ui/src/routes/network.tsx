import { createFileRoute } from "@tanstack/react-router";
import { NetworkPage } from "@/features/network/pages/NetworkPage";

export const Route = createFileRoute("/network")({
  component: NetworkPage,
  validateSearch: (search: Record<string, unknown>) => ({
    pid: search.pid ? Number(search.pid) : undefined,
  }),
});
