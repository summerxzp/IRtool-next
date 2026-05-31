import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/log-collector")({
  component: LogCollectorPage,
});

function LogCollectorPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.log-collector")}</h2>
      <p className="text-fg-secondary mt-2">P4 阶段实装</p>
    </div>
  );
}
