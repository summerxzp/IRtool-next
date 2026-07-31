import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

const CST_OFFSET_MS = 8 * 3600 * 1000;
const _pad2 = (n: number) => String(n).padStart(2, "0");

/** 格式化 epoch 秒为 UTC+8: YYYY/MM/DD,HH:MM:SS */
export function formatEpochSeconds(epoch: number): string {
  if (!epoch) return "-";
  const d = new Date(epoch * 1000 + CST_OFFSET_MS);
  return `${d.getUTCFullYear()}/${_pad2(d.getUTCMonth() + 1)}/${_pad2(d.getUTCDate())},${_pad2(d.getUTCHours())}:${_pad2(d.getUTCMinutes())}:${_pad2(d.getUTCSeconds())}`;
}

/** 格式化 epoch 毫秒为 UTC+8: YYYY/MM/DD,HH:MM:SS */
export function formatEpochMillis(epochMs: number): string {
  if (!epochMs) return "-";
  const d = new Date(epochMs + CST_OFFSET_MS);
  return `${d.getUTCFullYear()}/${_pad2(d.getUTCMonth() + 1)}/${_pad2(d.getUTCDate())},${_pad2(d.getUTCHours())}:${_pad2(d.getUTCMinutes())}:${_pad2(d.getUTCSeconds())}`;
}

export { formatEpochMillis as formatEventTimestamp };
