import { formatEpochMillis } from "@/lib/utils";

export function formatTimestamp(rfc3339: string | null | undefined): string {
  if (!rfc3339) return "";
  try {
    const ms = new Date(rfc3339).getTime();
    if (isNaN(ms)) return rfc3339;
    return formatEpochMillis(ms);
  } catch {
    return rfc3339;
  }
}
