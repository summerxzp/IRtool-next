import { createFileRoute } from "@tanstack/react-router";
import BackgroundMonitoringPage from "@/features/monitoring/pages/BackgroundMonitoringPage";

export const Route = createFileRoute("/background-monitoring")({
  component: BackgroundMonitoringPage,
});
