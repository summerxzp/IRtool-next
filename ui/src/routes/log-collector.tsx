import { createFileRoute } from "@tanstack/react-router";
import LogCollectorPage from "@/features/log-collector/pages/LogCollectorPage";

export const Route = createFileRoute("/log-collector")({
  component: LogCollectorPage,
});
