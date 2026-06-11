import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, RefreshCw, Shield, Settings, RotateCcw } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";

interface Props {
  onSearch: (query: string) => void;
  onRuleScan: () => void;
  onRuleManager: () => void;
  onRefresh: () => void;
  onReset: () => void;
  scanning: boolean;
}

export function WorkspaceToolbar({ onSearch, onRuleScan, onRuleManager, onRefresh, onReset, scanning }: Props) {
  const { t } = useTranslation();
  const [searchInput, setSearchInput] = useState("");

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && searchInput.trim()) {
      onSearch(searchInput.trim());
    }
  };

  return (
    <div className="flex items-center gap-2 p-2 bg-bg-elev-1 border-b border-border">
      <Input
        type="text"
        placeholder={t("workspace.toolbar.search-placeholder")}
        value={searchInput}
        onChange={(e) => setSearchInput(e.target.value)}
        onKeyDown={handleKeyDown}
        className="flex-1 max-w-xs"
      />
      <Button
        variant="secondary"
        size="sm"
        onClick={() => searchInput.trim() && onSearch(searchInput.trim())}
        disabled={!searchInput.trim()}
        className="hover:shadow-sm transition-shadow"
      >
        <Search className="h-3.5 w-3.5 mr-1" />
        {t("workspace.toolbar.search")}
      </Button>
      <Button
        variant="secondary"
        size="sm"
        onClick={() => { setSearchInput(""); onReset(); }}
      >
        <RotateCcw className="h-3.5 w-3.5 mr-1" />
        {t("workspace.toolbar.reset")}
      </Button>

      <div className="flex-1" />

      <Button variant="secondary" size="sm" onClick={onRuleScan} disabled={scanning} className="hover:shadow-sm transition-shadow">
        <Shield className="h-3.5 w-3.5 mr-1" />
        {t("workspace.toolbar.scan")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onRuleManager} className="hover:shadow-sm transition-shadow">
        <Settings className="h-3.5 w-3.5 mr-1" />
        {t("workspace.toolbar.rules")}
      </Button>
      <Button variant="secondary" size="sm" onClick={onRefresh} disabled={scanning} className="hover:shadow-sm transition-shadow">
        <RefreshCw className={`h-3.5 w-3.5 mr-1 ${scanning ? "animate-spin" : ""}`} />
        {t("workspace.toolbar.refresh")}
      </Button>
    </div>
  );
}
