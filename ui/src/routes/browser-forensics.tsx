import { createFileRoute } from "@tanstack/react-router";
import { BrowserForensicsPage } from "@/features/browser-forensics/pages/BrowserForensicsPage";

export const Route = createFileRoute("/browser-forensics")({
  component: BrowserForensicsPage,
});
