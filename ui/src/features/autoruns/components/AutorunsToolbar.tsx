import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Play, X, Download, Hash, ChevronDown } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
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

  const [searchInput, setSearchInput] = useState(filters.search);
  useEffect(() => {
    const id = setTimeout(() => setFilter("search", searchInput), 200);
    return () => clearTimeout(id);
  }, [searchInput, setFilter]);

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      {scanning ? (
        <Button variant="secondary" size="sm" onClick={onCancel} className="hover:shadow-sm transition-shadow">
          <X className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.cancel")}
        </Button>
      ) : (
        <Button variant="default" size="sm" onClick={onScan} className="hover:shadow-sm transition-shadow">
          <Play className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.scan")}
        </Button>
      )}

      <Popover>
        <PopoverTrigger asChild>
          <Button variant="secondary" size="sm" className="h-7 text-xs">
            {filters.categories.length === 0 ? t("autoruns.toolbar.all-categories") : `${t("autoruns.toolbar.category-label")} (${filters.categories.length})`}
            <ChevronDown className="h-3 w-3 ml-1" />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-48 p-2 max-h-60 overflow-y-auto" align="start">
          {categories.map((c) => (
            <div
              key={c}
              className="flex items-center gap-2 py-0.5 cursor-pointer hover:bg-bg-elev-2 rounded px-1"
              onClick={() => {
                const next = filters.categories.includes(c)
                  ? filters.categories.filter((v) => v !== c)
                  : [...filters.categories, c];
                setFilter("categories", next);
              }}
            >
              <Checkbox checked={filters.categories.includes(c)} />
              <Label className="text-xs cursor-pointer">{c}</Label>
            </div>
          ))}
        </PopoverContent>
      </Popover>

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

      <Input type="text" placeholder={t("autoruns.toolbar.search-placeholder")} value={searchInput} onChange={(e) => setSearchInput(e.target.value)} className="flex-1 min-w-0" />
      {searchInput && (<Button variant="ghost" size="icon" onClick={() => setSearchInput("")}><X className="h-3.5 w-3.5" /></Button>)}

      <Button variant="secondary" size="sm" onClick={onBatchCalculateHash} disabled={!hasData || calculatingHash} className="hover:shadow-sm transition-shadow">
        <Hash className="h-3.5 w-3.5 mr-1" />{calculatingHash ? t("autoruns.toolbar.calculating-hash") : t("autoruns.toolbar.batch-hash")}
      </Button>

      <Button variant="secondary" size="sm" onClick={onExport} disabled={!hasData} className="hover:shadow-sm transition-shadow">
        <Download className="h-3.5 w-3.5 mr-1" />{t("autoruns.toolbar.export-csv")}
      </Button>
    </div>
  );
}
