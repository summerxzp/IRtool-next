import { createFileRoute } from "@tanstack/react-router";
import { WorkspacePage } from "@/features/workspace/pages/WorkspacePage";

export const Route = createFileRoute("/workspace")({
  component: WorkspacePage,
});
