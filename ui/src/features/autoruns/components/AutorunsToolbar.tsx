import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Play, X, Download, Hash, PanelBottom, PanelRight } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useAutorunsStore } from "../store";

interface Props {
  onScan: () => void;
  onCancel: () => void;
  onBatchCalculateHash: () => void;
  onExport: () => void;
  scanning: boolean;
  calculatingHash: boolean;
  hasData: boolean;
  categories: string[];
}

const STATUS_OPTIONS: Array<"all" | "enabled" | "disabled"> = ["all", "enabled", "disabled"];
const SIGNATURE_OPTIONS: Array<"all" | "valid" | "invalid" | "unsigned"> = ["all", "valid", "invalid", "unsigned"];

export function AutorunsToolbar({ onScan, onCancel, onBatchCalculateHash, onExport, scanning, calculatingHash, hasData, categories }: Props) {
  const { t } = useTranslation();
  const filters = useAutorunsStore((s) => s.filters);
  const setFilter = useAutorunsStore((s) => s.setFilter);
  const detailPosition = useAutorunsStore((s) => s.detailPosition);
  const setDetailPosition = useAutorunsStore((s) => s.setDetailPosition);

  const [searchInput, setSearchInput] = useState(filters.search);
  useEffect(() => {
    const id = setTimeout(() => setFilter("search", searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setFilter]);

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      {scanning ? (
        <Button variant="secondary" size="sm" onClick={onCancel}>
          <X className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.cancel")}
        </Button>
      ) : (
        <Button variant="default" size="sm" onClick={onScan}>
          <Play className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.scan")}
        </Button>
      )}

      <Select value={filters.category} onValueChange={(v) => setFilter("category", v)}>
        <SelectTrigger className="h-7 w-32 text-xs"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{t("autoruns.toolbar.all-categories")}</SelectItem>
          {categories.map((c) => (<SelectItem key={c} value={c}>{c}</SelectItem>))}
        </SelectContent>
      </Select>

      <Select value={filters.status} onValueChange={(v) => setFilter("status", v as typeof filters.status)}>
        <SelectTrigger className="h-7 w-24 text-xs"><SelectValue /></SelectTrigger>
        <SelectContent>
          {STATUS_OPTIONS.map((s) => (<SelectItem key={s} value={s}>{t(`autoruns.toolbar.status-${s}`)}</SelectItem>))}
        </SelectContent>
      </Select>

      <Select value={filters.signature} onValueChange={(v) => setFilter("signature", v as typeof filters.signature)}>
        <SelectTrigger className="h-7 w-24 text-xs"><SelectValue /></SelectTrigger>
        <SelectContent>
          {SIGNATURE_OPTIONS.map((s) => (<SelectItem key={s} value={s}>{t(`autoruns.toolbar.signature-${s}`)}</SelectItem>))}
        </SelectContent>
      </Select>

      <Input type="text" placeholder={t("autoruns.toolbar.search-placeholder")} value={searchInput} onChange={(e) => setSearchInput(e.target.value)} className="flex-1 max-w-xs" />
      {searchInput && (<Button variant="ghost" size="icon" onClick={() => setSearchInput("")}><X className="h-3.5 w-3.5" /></Button>)}

      <div className="flex-1" />

      <Button variant="ghost" size="icon" onClick={() => setDetailPosition(detailPosition === "bottom" ? "right" : "bottom")} title={detailPosition === "bottom" ? t("autoruns.toolbar.detail-right") : t("autoruns.toolbar.detail-bottom")}>
        {detailPosition === "bottom" ? <PanelRight className="h-3.5 w-3.5" /> : <PanelBottom className="h-3.5 w-3.5" />}
      </Button>

      <Button variant="secondary" size="sm" onClick={onBatchCalculateHash} disabled={!hasData || calculatingHash}>
        <Hash className="h-3.5 w-3.5 mr-1" />{calculatingHash ? t("autoruns.toolbar.calculating-hash") : t("autoruns.toolbar.batch-hash")}
      </Button>

      <Button variant="secondary" size="sm" onClick={onExport} disabled={!hasData}>
        <Download className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.export-csv")}
      </Button>
    </div>
  );
}
