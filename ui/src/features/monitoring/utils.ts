import { formatEpochMillis } from "@/lib/utils";

export function formatUptime(startedAt: number | null): string {
  if (!startedAt || startedAt <= 0) return "-";
  const ms = Date.now() - startedAt;
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}秒`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}分${secs % 60}秒`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}时${mins % 60}分`;
  const days = Math.floor(hours / 24);
  return `${days}天${hours % 24}时`;
}

export function formatTimestamp(epochMs: number | null): string {
  if (!epochMs) return "-";
  return formatEpochMillis(epochMs);
}
