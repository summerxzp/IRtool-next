import { useTranslation } from "react-i18next";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import type { AutorunItem } from "../types";

interface Props {
  item: AutorunItem | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (entryId: number) => void;
}

export function DeleteEntryDialog({ item, open, onOpenChange, onConfirm }: Props) {
  const { t } = useTranslation();
  if (!item) return null;

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("autoruns.delete-confirm.title")}</AlertDialogTitle>
          <AlertDialogDescription>
            {t("autoruns.delete-confirm.message", { entry: item.entry, category: item.category })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
          <AlertDialogAction className="bg-danger text-white hover:bg-danger/90" onClick={() => { onConfirm(item.id); onOpenChange(false); }}>
            {t("autoruns.delete-confirm.confirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
