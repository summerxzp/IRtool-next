import { createFileRoute } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export const Route = createFileRoute("/autoruns")({
  component: AutorunsPage,
});

function AutorunsPage() {
  const { t } = useTranslation();
  return (
    <div className="p-6">
      <h2 className="text-lg font-semibold">{t("nav.autoruns")}</h2>
      <p className="text-fg-secondary mt-2">P2 阶段实装</p>
    </div>
  );
}
