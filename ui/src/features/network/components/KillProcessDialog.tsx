import { useTranslation } from "react-i18next";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import type { NetConn } from "../types";

interface Props {
  conn: NetConn | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (pid: number) => void;
}

export function KillProcessDialog({ conn, open, onOpenChange, onConfirm }: Props) {
  const { t } = useTranslation();

  if (!conn) return null;

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("network.kill-confirm.title")}</AlertDialogTitle>
          <AlertDialogDescription>
            {t("network.kill-confirm.message", {
              pid: conn.pid,
              name: conn.process_name ?? "?",
            })}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("network.kill-confirm.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            className="bg-danger text-white hover:bg-danger/90"
            onClick={() => {
              onConfirm(conn.pid);
              onOpenChange(false);
            }}
          >
            {t("network.kill-confirm.confirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
