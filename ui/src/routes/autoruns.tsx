import { createFileRoute } from "@tanstack/react-router";
import { AutorunsPage } from "@/features/autoruns/pages/AutorunsPage";

export const Route = createFileRoute("/autoruns")({
  component: AutorunsPage,
  validateSearch: (search: Record<string, unknown>) => ({
    imagePath: search.imagePath as string | undefined,
  }),
});
