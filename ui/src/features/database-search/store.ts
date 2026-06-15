import { create } from "zustand";

export interface DbSearchEvent {
  id: number;
  event_type: string;
  timestamp: string;
  timestamp_epoch: number;
  record_id: number;
  process_id: number;
  process_name: string;
  process_path: string;
  user: string;
  query_name: string;
  query_results: string;
  query_status: number;
  source_ip: string;
  source_port: number;
  destination_ip: string;
  destination_port: number;
  protocol: string;
  initiated: boolean;
  is_external: boolean;
  source_process_id: number;
  source_process_name: string;
  source_process_path: string;
  target_process_id: number;
  target_process_name: string;
  target_process_path: string;
  start_address: string;
  start_module: string;
  start_function: string;
  is_suspicious: boolean;
  target_filename: string;
  creation_utc_time: string;
  _rawJson: string;
  _source: string;
  [key: string]: any;
}

interface DbSearchState {
  events: DbSearchEvent[];
  selectedEvent: DbSearchEvent | null;
  totalCount: number; // 数据库总记录数
  matchedCount: number; // 搜索匹配的记录数
  hasFilters: boolean; // 是否有搜索条件
  setEvents: (events: DbSearchEvent[]) => void;
  appendEvents: (events: DbSearchEvent[]) => void;
  setSelectedEvent: (event: DbSearchEvent | null) => void;
  setTotalCount: (count: number) => void;
  setMatchedCount: (count: number) => void;
  setHasFilters: (hasFilters: boolean) => void;
  clear: () => void;
}

export const useDbSearchStore = create<DbSearchState>((set) => ({
  events: [],
  selectedEvent: null,
  totalCount: 0,
  matchedCount: 0,
  hasFilters: false,
  setEvents: (events) => set({ events }),
  appendEvents: (newEvents) =>
    set((state) => ({ events: [...state.events, ...newEvents] })),
  setSelectedEvent: (event) => set({ selectedEvent: event }),
  setTotalCount: (count) => set({ totalCount: count }),
  setMatchedCount: (count) => set({ matchedCount: count }),
  setHasFilters: (hasFilters) => set({ hasFilters }),
  clear: () => set({ events: [], selectedEvent: null, totalCount: 0, matchedCount: 0, hasFilters: false }),
}));
