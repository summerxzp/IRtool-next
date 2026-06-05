import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQuery, useMutation } from "@tanstack/react-query";
import * as api from "./api";
import { useLogCollectorStore } from "./store";
import { DEFAULT_ENABLED_EVENT_IDS } from "./types";
import type { SysmonEvent } from "./types";

const EVT_SYSMON_EVENT = "evt_sysmon_event";

export function useSysmonStatus() {
  return useQuery({
    queryKey: ["sysmon", "status"],
    queryFn: api.getStatus,
    refetchInterval: 5000,
  });
}

export function useDefaultEventConfigs() {
  return useQuery({
    queryKey: ["sysmon", "event-configs"],
    queryFn: api.getDefaultEventConfigs,
  });
}

export function useLoadHistory() {
  return useMutation({
    mutationFn: (limit: number) => api.getExistingEvents(limit, DEFAULT_ENABLED_EVENT_IDS),
  });
}

export function useInstallSysmon() {
  return useMutation({
    mutationFn: (acceptEula: boolean) => api.install(acceptEula),
  });
}

export function useUninstallSysmon() {
  return useMutation({
    mutationFn: () => api.uninstall(),
  });
}

export function useUpdateSysmonConfig() {
  return useMutation({
    mutationFn: () => api.updateConfig(),
  });
}

export function useSysmonEventListener() {
  const addEvents = useLogCollectorStore((s) => s.addEvents);
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    const setup = async () => {
      unlistenRef.current = await listen<SysmonEvent>(EVT_SYSMON_EVENT, (event) => {
        addEvents([event.payload]);
      });
    };
    setup();
    return () => {
      unlistenRef.current?.();
    };
  }, [addEvents]);
}

export function useStartCollection() {
  const setCollecting = useLogCollectorStore((s) => s.setCollecting);
  const setStartTime = useLogCollectorStore((s) => s.setStartTime);
  const setAutoScroll = useLogCollectorStore((s) => s.setAutoScroll);

  return useMutation({
    mutationFn: async () => {
      await api.startSubscription(DEFAULT_ENABLED_EVENT_IDS, 500);
    },
    onSuccess: () => {
      setCollecting(true);
      setStartTime(Date.now());
      setAutoScroll(true);
    },
  });
}

export function useStopCollection() {
  const setCollecting = useLogCollectorStore((s) => s.setCollecting);
  const setStartTime = useLogCollectorStore((s) => s.setStartTime);

  return useMutation({
    mutationFn: async () => {
      await api.stopSubscription();
    },
    onSuccess: () => {
      setCollecting(false);
      setStartTime(null);
    },
  });
}
