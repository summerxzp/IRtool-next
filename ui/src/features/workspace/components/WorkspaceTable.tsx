import { useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import { useWorkspaceStore } from "../store";
import { networkKey as engineNetworkKey, eventKey as engineEventKey } from "../rules/engine";
import type { AutorunItem } from "@/features/autoruns/types";
import type { NetConn } from "@/features/network/types";
import type { SysmonEvent } from "@/features/log-collector/types";

export function netConnKey(item: NetConn): string {
  return engineNetworkKey(item);
}

export function eventKey(item: SysmonEvent): string {
  return engineEventKey(item);
}

interface Props {
  onRowSelect: (key: string | null) => void;
}

export function WorkspaceTable({ onRowSelect }: Props) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);

  const activeTab = useWorkspaceStore((s) => s.activeTab);
  const autorunItems = useWorkspaceStore((s) => s.autorunItems);
  const networkItems = useWorkspaceStore((s) => s.networkItems);
  const eventItems = useWorkspaceStore((s) => s.eventItems);
  const filteredAutorunIds = useWorkspaceStore((s) => s.filteredAutorunIds);
  const filteredNetworkKeys = useWorkspaceStore((s) => s.filteredNetworkKeys);
  const filteredEventKeys = useWorkspaceStore((s) => s.filteredEventKeys);
  const autorunMatchedRules = useWorkspaceStore((s) => s.autorunMatchedRules);
  const networkMatchedRules = useWorkspaceStore((s) => s.networkMatchedRules);
  const eventMatchedRules = useWorkspaceStore((s) => s.eventMatchedRules);
  const selectedAutorunId = useWorkspaceStore((s) => s.selectedAutorunId);
  const selectedNetworkKey = useWorkspaceStore((s) => s.selectedNetworkKey);
  const selectedEventKey = useWorkspaceStore((s) => s.selectedEventKey);
  const setSelectedAutorunId = useWorkspaceStore((s) => s.setSelectedAutorunId);
  const setSelectedNetworkKey = useWorkspaceStore((s) => s.setSelectedNetworkKey);
  const setSelectedEventKey = useWorkspaceStore((s) => s.setSelectedEventKey);

  const rows = useMemo(() => {
    switch (activeTab) {
      case "autoruns": {
        const items = filteredAutorunIds
          ? autorunItems.filter((i) => filteredAutorunIds.has(i.id))
          : autorunItems;
        return items.map((item) => ({
          key: String(item.id),
          item,
          hasMatch: autorunMatchedRules.has(item.id),
        }));
      }
      case "network": {
        const items = filteredNetworkKeys
          ? networkItems.filter((i) => filteredNetworkKeys.has(netConnKey(i)))
          : networkItems;
        return items.map((item) => ({
          key: netConnKey(item),
          item,
          hasMatch: networkMatchedRules.has(netConnKey(item)),
        }));
      }
      case "events": {
        const items = filteredEventKeys
          ? eventItems.filter((i) => filteredEventKeys.has(eventKey(i)))
          : eventItems;
        return items.map((item) => ({
          key: eventKey(item),
          item,
          hasMatch: eventMatchedRules.has(eventKey(item)),
        }));
      }
    }
  }, [activeTab, autorunItems, networkItems, eventItems, filteredAutorunIds, filteredNetworkKeys, filteredEventKeys, autorunMatchedRules, networkMatchedRules, eventMatchedRules]);

  const selectedKey = useMemo(() => {
    switch (activeTab) {
      case "autoruns": return selectedAutorunId != null ? String(selectedAutorunId) : null;
      case "network": return selectedNetworkKey;
      case "events": return selectedEventKey;
    }
  }, [activeTab, selectedAutorunId, selectedNetworkKey, selectedEventKey]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => 28,
    overscan: 12,
  });

  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();

  const handleRowClick = (key: string) => {
    switch (activeTab) {
      case "autoruns":
        setSelectedAutorunId(selectedAutorunId != null && String(selectedAutorunId) === key ? null : Number(key));
        break;
      case "network":
        setSelectedNetworkKey(selectedNetworkKey === key ? null : key);
        break;
      case "events":
        setSelectedEventKey(selectedEventKey === key ? null : key);
        break;
    }
    onRowSelect(key);
  };

  const columns = useMemo(() => {
    switch (activeTab) {
      case "autoruns":
        return [
          { key: "entry", label: t("workspace.columns.entry"), width: 200 },
          { key: "image_path", label: t("workspace.columns.image-path"), width: 300 },
          { key: "category", label: t("workspace.columns.category"), width: 120 },
          { key: "risk", label: t("workspace.columns.risk"), width: 80 },
        ];
      case "network":
        return [
          { key: "proto", label: t("workspace.columns.proto"), width: 60 },
          { key: "remote", label: t("workspace.columns.remote"), width: 220 },
          { key: "state", label: t("workspace.columns.state"), width: 100 },
          { key: "pid", label: t("workspace.columns.pid"), width: 60 },
          { key: "process_name", label: t("workspace.columns.process"), width: 150 },
        ];
      case "events":
        return [
          { key: "event_type", label: t("workspace.columns.type"), width: 120 },
          { key: "timestamp", label: t("workspace.columns.time"), width: 160 },
          { key: "destination", label: t("workspace.columns.destination"), width: 220 },
          { key: "process_name", label: t("workspace.columns.process"), width: 150 },
        ];
    }
  }, [activeTab, t]);

  const getCellValue = (item: AutorunItem | NetConn | SysmonEvent, colKey: string): string => {
    switch (activeTab) {
      case "autoruns": {
        const a = item as AutorunItem;
        switch (colKey) {
          case "entry": return a.entry;
          case "image_path": return a.image_path ?? "";
          case "category": return a.category;
          case "risk": return a.risk;
          default: return "";
        }
      }
      case "network": {
        const n = item as NetConn;
        switch (colKey) {
          case "proto": return n.proto.toUpperCase();
          case "remote": return `${n.remote.addr}:${n.remote.port}`;
          case "state": return n.state;
          case "pid": return String(n.pid);
          case "process_name": return n.process_name ?? "";
          default: return "";
        }
      }
      case "events": {
        const e = item as SysmonEvent;
        switch (colKey) {
          case "event_type": return e.event_type;
          case "timestamp": return e.timestamp;
          case "destination": return e.query_name || e.destination_ip || e.target_filename || "";
          case "process_name": return e.process_name;
          default: return "";
        }
      }
    }
  };

  if (rows.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-fg-tertiary text-sm">
        {t("workspace.table.no-results")}
      </div>
    );
  }

  const paddingTop = virtualItems[0]?.start ?? 0;
  const paddingBottom = virtualItems.length > 0
    ? totalSize - (virtualItems[virtualItems.length - 1]?.end ?? 0)
    : 0;

  return (
    <div ref={containerRef} className="h-full w-full overflow-auto bg-bg-base">
      <table className="w-full text-sm font-sans border-collapse">
        <thead className="sticky top-0 z-10 bg-bg-elev-1">
          <tr className="border-b border-border">
            {columns.map((col) => (
              <th
                key={col.key}
                className="h-7 px-2 text-left font-medium text-fg-secondary text-xs select-none"
                style={{ width: col.width, minWidth: 40 }}
              >
                {col.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {paddingTop > 0 && (
            <tr style={{ height: paddingTop }}>
              <td colSpan={columns.length} />
            </tr>
          )}
          {virtualItems.map((vRow) => {
            const row = rows[vRow.index];
            const isSelected = row.key === selectedKey;
            return (
              <tr
                key={row.key}
                className={`border-b border-border/50 cursor-pointer transition-colors ${
                  isSelected ? "bg-bg-elev-2" : "hover:bg-bg-elev-2/40"
                }`}
                style={{ height: 28 }}
                onClick={() => handleRowClick(row.key)}
              >
                {columns.map((col, ci) => (
                  <td
                    key={col.key}
                    className={`px-2 truncate text-sm ${
                      ci === 0 && row.hasMatch ? "border-l-2 border-l-danger text-fg-primary" : "text-fg-primary"
                    }`}
                    style={{ width: col.width }}
                  >
                    {getCellValue(row.item, col.key)}
                  </td>
                ))}
              </tr>
            );
          })}
          {paddingBottom > 0 && (
            <tr style={{ height: paddingBottom }}>
              <td colSpan={columns.length} />
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
