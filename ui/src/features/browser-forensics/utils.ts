export function formatTimestamp(rfc3339: string | null | undefined): string {
  if (!rfc3339) return "";
  try {
    const d = new Date(rfc3339);
    if (isNaN(d.getTime())) return rfc3339;
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    const h = String(d.getHours()).padStart(2, "0");
    const min = String(d.getMinutes()).padStart(2, "0");
    const s = String(d.getSeconds()).padStart(2, "0");
    return `${y}/${m}/${day} ${h}:${min}:${s}`;
  } catch {
    return rfc3339;
  }
}
