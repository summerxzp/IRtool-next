import { createFileRoute } from "@tanstack/react-router";
import { ProcessPage } from "@/features/process/pages/ProcessPage";

export const Route = createFileRoute("/process")({
  component: ProcessPage,
  validateSearch: (search: Record<string, unknown>) => ({
    pid: search.pid ? Number(search.pid) : undefined,
    imagePath: search.imagePath as string | undefined,
  }),
});
