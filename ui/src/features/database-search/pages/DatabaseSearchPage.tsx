import { useState, useCallback, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Panel, Group, Separator } from "react-resizable-panels";
import { Copy, X, Trash2, Download } from "lucide-react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import * as api from "@/features/log-collector/api";
import { monitorEventToSysmonEvent } from "@/features/log-collector/pages/LogCollectorPage";
import { useDbSearchStore, type DbSearchEvent } from "../store";
import { exportCsv } from "@/lib/csv";
import { EVENT_TYPE_LABELS, EVENT_TYPE_COLORS } from "@/features/log-collector/types";
import type { ExtendedSysmonEventType } from "@/features/log-collector/types";
import { DataTable } from "@/components/data-table/DataTable";
import { type ColumnDef } from "@tanstack/react-table";

type EventSource = "sysmon" | "dns_client" | "pcap" | "net_monitor" | "all";

function getDestination(event: DbSearchEvent): string {
  const et = event.event_type as ExtendedSysmonEventType;
  switch (et) {
    case "network_connect":
      return `${event.destination_ip}:${event.destination_port}`;
    case "dns":
    case "dns_client":
    case "tls_sni":
    case "dns_pcap":
      return event.query_name || "-";
    case "create_remote_thread":
      return `${event.source_process_name} → ${event.target_process_name}`;
    case "file_create":
      return event.target_filename || "-";
    default:
      return event.process_name || "-";
  }
}

// --- 独立的事件详情组件 ---
function FieldRow({ label, value, copyable }: { label: string; value: string; copyable?: boolean }) {
  if (!value) return null;
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="text-fg-tertiary w-20 shrink-0 text-right">{label}</span>
      <span className="text-fg-primary break-all flex-1">{value}</span>
      {copyable && (
        <Button variant="ghost" size="icon" className="h-5 w-5 shrink-0" onClick={() => navigator.clipboard.writeText(value)}>
          <Copy className="h-3 w-3" />
        </Button>
      )}
    </div>
  );
}

function DbEventDetail({ event, onClose }: { event: DbSearchEvent | null; onClose?: () => void }) {
  if (!event) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-fg-tertiary">
        选择事件查看详情
      </div>
    );
  }

  const et = event.event_type as ExtendedSysmonEventType;

  return (
    <div className="p-3 space-y-2 overflow-auto h-full">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{EVENT_TYPE_LABELS[et] || event.event_type}</span>
        </div>
        {onClose && (
          <button className="text-fg-tertiary hover:text-fg-primary shrink-0 p-0.5" onClick={onClose}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
      <Separator />

      <FieldRow label="时间" value={event.timestamp} />
      <FieldRow label="进程" value={event.process_name && event.process_name !== "<unknown process>" ? `${event.process_name} (${event.process_id})` : `PID: ${event.process_id}`} />
      <FieldRow label="路径" value={event.process_path === "<unknown process>" ? "" : event.process_path} copyable />
      <FieldRow label="用户" value={event.user} />

      {(et === "dns" || et === "dns_client") && (
        <>
          <Separator />
          <FieldRow label="域名" value={event.query_name} copyable />
          <FieldRow label="解析结果" value={event.query_results} copyable />
          <FieldRow label="状态" value={event.query_status > 0 ? String(event.query_status) : "0 (Success)"} />
        </>
      )}

      {(et === "tls_sni" || et === "dns_pcap") && (
        <>
          <Separator />
          <FieldRow label="域名" value={event.query_name} copyable />
          {event.query_results && <FieldRow label="结果" value={event.query_results} copyable />}
          <FieldRow label="来源" value={`${event.source_ip}:${event.source_port}`} />
          <FieldRow label="目标" value={`${event.destination_ip}:${event.destination_port}`} copyable />
          <FieldRow label="协议" value={event.protocol} />
        </>
      )}

      {et === "network_connect" && (
        <>
          <Separator />
          <FieldRow label="来源" value={`${event.source_ip}:${event.source_port}`} />
          <FieldRow label="目标" value={`${event.destination_ip}:${event.destination_port}`} copyable />
          <FieldRow label="协议" value={event.protocol} />
          <FieldRow label="方向" value={event.initiated ? "出站" : "入站"} />
          <FieldRow label="外部" value={event.is_external ? "是" : "否"} />
        </>
      )}

      {et === "network_monitor" && (
        <>
          <Separator />
          <FieldRow label="协议" value={event.protocol} />
          <FieldRow label="状态" value={(event as any)._state || ""} />
          <FieldRow label="来源" value={`${event.source_ip}:${event.source_port}`} />
          <FieldRow label="目标" value={`${event.destination_ip}:${event.destination_port}`} copyable />
        </>
      )}

      {et === "create_remote_thread" && (
        <>
          <Separator />
          <FieldRow label="源进程" value={`${event.source_process_name} (${event.source_process_id})`} />
          <FieldRow label="源路径" value={event.source_process_path} copyable />
          <FieldRow label="目标进程" value={`${event.target_process_name} (${event.target_process_id})`} />
          <FieldRow label="目标路径" value={event.target_process_path} copyable />
          <FieldRow label="起始地址" value={event.start_address} copyable />
          <FieldRow label="起始模块" value={event.start_module} />
        </>
      )}

      {et === "file_create" && (
        <>
          <Separator />
          <FieldRow label="文件名" value={event.target_filename} copyable />
          <FieldRow label="创建时间" value={event.creation_utc_time} />
        </>
      )}

      <Separator />
      <div className="text-[10px] text-fg-tertiary">来源: {event._source} | DB ID: {event.record_id}</div>
    </div>
  );
}

// --- 主页面 ---
export default function DatabaseSearchPage() {
  const { t } = useTranslation();
  const [source, setSource] = useState<EventSource>("all");
  const [eventType, setEventType] = useState<string>("all");
  const [processName, setProcessName] = useState<string>("");
  const [keyField, setKeyField] = useState<string>("");
  const [searchText, setSearchText] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [offset, setOffset] = useState(0);
  const [loadLimit, setLoadLimit] = useState(1000);
  const [clearConfirmOpen, setClearConfirmOpen] = useState(false);
  const [typeCounts, setTypeCounts] = useState<Record<string, number>>({});

  const { events, selectedEvent, setEvents, appendEvents, setSelectedEvent, totalCount, setTotalCount, clear } = useDbSearchStore();

  useEffect(() => {
    api.monitorGetConfig().then((c) => {
      setLoadLimit(c.load_limit);
    }).catch(() => {});
  }, []);

  // 页面卸载时清空数据
  useEffect(() => {
    return () => { clear(); };
  }, [clear]);

  const doSearch = useCallback(async (newOffset = 0) => {
    setLoading(true);
    try {
      const [dbEvents, count, countsArr] = await Promise.all([
        api.monitorSearchEvents(
          source === "all" ? undefined : source,
          eventType === "all" ? undefined : eventType,
          processName || undefined,
          keyField || undefined,
          searchText || undefined,
          loadLimit,
          newOffset,
        ),
        api.monitorGetEventCount(),
        invoke<[string, number][]>("cmd_monitor_event_type_counts"),
      ]);
      setTotalCount(count);
      const tc: Record<string, number> = {};
      for (const [k, v] of countsArr) { tc[k] = v; }
      setTypeCounts(tc);
      const convertedEvents = dbEvents.map(monitorEventToSysmonEvent);
      if (newOffset === 0) {
        setEvents(convertedEvents);
      } else {
        appendEvents(convertedEvents);
      }
      setOffset(newOffset + dbEvents.length);
      setHasMore(dbEvents.length === loadLimit);
      toast.success(`数据库中总计 ${count} 条记录，已加载 ${newOffset + convertedEvents.length} 条`);
    } catch (e) {
      toast.error("搜索事件失败", { description: e instanceof Error ? e.message : "未知错误" });
    } finally {
      setLoading(false);
    }
  }, [source, eventType, processName, keyField, searchText, loadLimit, setEvents, appendEvents, setTotalCount]);

  const handleSearch = useCallback(() => {
    doSearch(0);
  }, [doSearch]);

  const handleLoadMore = useCallback(() => {
    doSearch(offset);
  }, [doSearch, offset]);

  const handleReset = useCallback(() => {
    setSource("all");
    setEventType("all");
    setProcessName("");
    setKeyField("");
    setSearchText("");
    clear();
    setOffset(0);
    setHasMore(false);
    setTypeCounts({});
  }, [clear]);

  const handleExportCsv = useCallback(async () => {
    const rows = events.map((e) => ({
      timestamp: e.timestamp,
      source: e._source,
      event_type: e.event_type,
      process_id: e.process_id,
      process_name: e.process_name,
      process_path: e.process_path,
      user: e.user,
      source_ip: e.source_ip,
      source_port: e.source_port,
      destination_ip: e.destination_ip,
      destination_port: e.destination_port,
      protocol: e.protocol,
      initiated: e.initiated ? "是" : "否",
      is_external: e.is_external ? "是" : "否",
      query_name: e.query_name,
      query_results: e.query_results,
      source_process_name: e.source_process_name,
      source_process_path: e.source_process_path,
      target_process_name: e.target_process_name,
      target_process_path: e.target_process_path,
      start_address: e.start_address,
      target_filename: e.target_filename,
      raw_json: e._rawJson,
    }));
    await exportCsv(rows, [
      "timestamp", "source", "event_type", "process_id", "process_name", "process_path",
      "user", "source_ip", "source_port", "destination_ip", "destination_port",
      "protocol", "initiated", "is_external", "query_name", "query_results",
      "source_process_name", "source_process_path", "target_process_name",
      "target_process_path", "start_address", "target_filename", "raw_json",
    ], "database-search.csv");
  }, [events]);

  const dbColumns = useMemo<ColumnDef<DbSearchEvent, unknown>[]>(() => [
    {
      accessorKey: "timestamp",
      header: "时间",
      size: 144,
      cell: ({ getValue }) => <span className="font-mono text-fg-secondary whitespace-nowrap overflow-hidden text-ellipsis">{getValue() as string || "-"}</span>,
    },
    {
      accessorKey: "event_type",
      header: "类型",
      size: 96,
      cell: ({ getValue }) => {
        const et = getValue() as string;
        return <span className={`inline-flex items-center px-1.5 py-0 rounded-sm text-[10px] font-medium whitespace-nowrap ${EVENT_TYPE_COLORS[et as ExtendedSysmonEventType] || ""}`}>
          {EVENT_TYPE_LABELS[et as ExtendedSysmonEventType] || et}
        </span>;
      },
    },
    {
      id: "destination",
      accessorFn: (row) => getDestination(row),
      header: "目标",
      size: 176,
      cell: ({ row }) => <span className="truncate text-fg-primary" title={getDestination(row.original)}>{getDestination(row.original)}</span>,
    },
    {
      accessorKey: "process_path",
      header: "路径",
      size: 300,
      cell: ({ getValue }) => <span className="truncate text-fg-secondary" title={getValue() as string}>{getValue() as string}</span>,
    },
  ], []);

  return (
    <div className="flex flex-col h-full">
      <div className="flex flex-wrap gap-4 p-3 border-b border-border bg-bg-elevated-1">
        <div className="flex flex-col gap-1">
          <Label className="text-xs">来源</Label>
          <Select value={source} onValueChange={(v: EventSource) => setSource(v)}>
            <SelectTrigger className="h-7 w-36 text-xs">
              <SelectValue placeholder="来源" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部</SelectItem>
              <SelectItem value="sysmon">Sysmon</SelectItem>
              <SelectItem value="dns_client">DNS Client</SelectItem>
              <SelectItem value="pcap">Pcap</SelectItem>
              <SelectItem value="net_monitor">网络监控</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label className="text-xs">事件类型</Label>
          <Select value={eventType} onValueChange={setEventType}>
            <SelectTrigger className="h-7 w-36 text-xs">
              <SelectValue placeholder="事件类型" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">全部</SelectItem>
              <SelectItem value="dns">DNS</SelectItem>
              <SelectItem value="dns_client">DNS Client</SelectItem>
              <SelectItem value="network_connect">网络连接</SelectItem>
              <SelectItem value="network_monitor">网络监控</SelectItem>
              <SelectItem value="create_remote_thread">远程线程</SelectItem>
              <SelectItem value="file_create">文件创建</SelectItem>
              <SelectItem value="tls_sni">TLS SNI</SelectItem>
              <SelectItem value="dns_pcap">DNS 抓包</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1">
          <Label className="text-xs">进程名</Label>
          <Input type="text" value={processName} onChange={(e) => setProcessName(e.target.value)} placeholder="模糊匹配" className="h-7 w-32 text-xs" />
        </div>
        <div className="flex flex-col gap-1">
          <Label className="text-xs">IP/域名</Label>
          <Input type="text" value={keyField} onChange={(e) => setKeyField(e.target.value)} placeholder="模糊匹配" className="h-7 w-32 text-xs" />
        </div>
        <div className="flex flex-col gap-1">
          <Label className="text-xs">全文搜索</Label>
          <Input type="text" value={searchText} onChange={(e) => setSearchText(e.target.value)} placeholder="搜索 raw_json" className="h-7 w-48 text-xs" />
        </div>
        <div className="flex items-end gap-2">
          <Button variant="default" size="sm" onClick={handleSearch} disabled={loading} className="h-7 text-xs hover:shadow-sm transition-shadow">
            {loading ? "搜索中..." : "搜索"}
          </Button>
          <Button variant="ghost" size="sm" onClick={handleReset} disabled={loading} className="h-7 text-xs">
            重置
          </Button>
          {hasMore && (
            <Button variant="secondary" size="sm" onClick={handleLoadMore} disabled={loading} className="h-7 text-xs hover:shadow-sm transition-shadow">
              加载更多
            </Button>
          )}
          <div className="flex-1" />
          <Button variant="secondary" size="sm" onClick={handleExportCsv} disabled={loading || events.length === 0} className="h-7 text-xs hover:shadow-sm transition-shadow">
            <Download className="h-3 w-3 mr-1" />
            {t("database-search.export-csv")}
          </Button>
          <Button variant="destructive" size="sm" onClick={() => setClearConfirmOpen(true)} disabled={loading} className="h-7 text-xs">
            <Trash2 className="h-3 w-3 mr-1" />
            清空
          </Button>
        </div>
      </div>
      <div className="flex-1 min-h-0">
        <Group orientation="horizontal">
          <Panel defaultSize={70} minSize={40}>
            <DataTable
              columns={dbColumns}
              data={events}
              getRowId={(e) => `${e.record_id}-${e.timestamp}`}
              onRowSelect={(row) => setSelectedEvent(row)}
              selectedRowId={selectedEvent ? `${selectedEvent.record_id}-${selectedEvent.timestamp}` : null}
              empty="点击搜索加载数据库事件"
              persistKey="db-search"
            />
          </Panel>
          {selectedEvent != null && (
            <>
              <Separator className="w-px bg-border hover:bg-accent transition-colors" />
              <Panel defaultSize={30} minSize={20}>
                <DbEventDetail event={selectedEvent} onClose={() => setSelectedEvent(null)} />
              </Panel>
            </>
          )}
        </Group>
      </div>
      <div className="flex items-center gap-3 px-3 py-1.5 border-t border-border bg-bg-elev-1 text-[10px] text-fg-tertiary">
        <span>总计 {totalCount} 条</span>
        {Object.entries(typeCounts).map(([type, count]) => (
          <span key={type} className="flex items-center gap-0.5">
            <span className="text-fg-secondary">{EVENT_TYPE_LABELS[type as ExtendedSysmonEventType] || type}</span>
            <span>{count}</span>
          </span>
        ))}
      </div>
      <Dialog open={clearConfirmOpen} onOpenChange={setClearConfirmOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>确认清空数据库</DialogTitle>
            <DialogDescription>将删除数据库中所有事件记录。此操作不可撤销。</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" size="sm" onClick={() => setClearConfirmOpen(false)}>取消</Button>
            <Button variant="destructive" size="sm" onClick={async () => {
              setClearConfirmOpen(false);
              try {
                await invoke("cmd_monitor_clear_events");
                clear();
                setOffset(0);
                setHasMore(false);
                toast.success("数据库已清空");
              } catch (e) {
                toast.error("清空失败", { description: e instanceof Error ? e.message : "未知错误" });
              }
            }}>清空</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
