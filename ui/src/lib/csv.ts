import Papa from "papaparse";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

export async function exportCsv<T extends Record<string, unknown>>(
  rows: T[],
  fields: Array<keyof T>,
  defaultFilename: string,
): Promise<{ saved: boolean; path?: string }> {
  if (rows.length === 0) {
    return { saved: false };
  }

  const path = await save({
    title: "Export CSV",
    defaultPath: defaultFilename,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });

  if (!path) return { saved: false };

  const projected = rows.map((r) => {
    const out: Record<string, unknown> = {};
    for (const f of fields) {
      out[String(f)] = r[f] ?? "";
    }
    return out;
  });

  const csv = Papa.unparse(projected, { quotes: true });
  await writeTextFile(path, csv);
  return { saved: true, path };
}
