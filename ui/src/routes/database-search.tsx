import { createFileRoute } from "@tanstack/react-router";
import DatabaseSearchPage from "@/features/database-search/pages/DatabaseSearchPage";

export const Route = createFileRoute("/database-search")({
  component: DatabaseSearchPage,
});
