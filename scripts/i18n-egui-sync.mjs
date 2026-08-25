#!/usr/bin/env node
/**
 * 同步 React locale 键表到 egui 前端（rust-i18n）。
 *
 * 用法：node scripts/i18n-egui-sync.mjs
 *
 * 流程：
 *   1. 读取 ui/src/locales/{zh-CN,en-US}.json（基准键表，键名禁改名）；
 *   2. 深合并 scripts/egui-locales-extra/{zh-CN,en-US}.json
 *      （egui 壳专属文本：shell.*，React 侧没有对应键）；
 *   3. 写出 crates/irtool-egui/locales/{zh-CN,en-US}.json
 *      （嵌套结构原样保留——rust-i18n 会自动展平为点号键）；
 *   4. 校验 zh/en 叶子键集一致并打印差集与计数。
 *
 * 已知源差异：React en-US 缺 browser-forensics.detail.ioc-matches，
 * 由 rust-i18n fallback = "zh-CN" 运行时兜底（本脚本如实报告，不代填译文）。
 */
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = join(root, "ui", "src", "locales");
const extraDir = join(root, "scripts", "egui-locales-extra");
const outDir = join(root, "crates", "irtool-egui", "locales");
const LOCALES = ["zh-CN", "en-US"];

const isObj = (v) => typeof v === "object" && v !== null && !Array.isArray(v);

function deepMerge(base, over) {
  const out = { ...base };
  for (const [k, v] of Object.entries(over)) {
    out[k] = isObj(v) && isObj(base[k]) ? deepMerge(base[k], v) : v;
  }
  return out;
}

function flatKeys(obj, prefix = "") {
  const keys = [];
  for (const [k, v] of Object.entries(obj)) {
    const full = prefix ? `${prefix}.${k}` : k;
    if (isObj(v)) keys.push(...flatKeys(v, full));
    else keys.push(full);
  }
  return keys;
}

const report = {};
for (const locale of LOCALES) {
  const base = JSON.parse(readFileSync(join(srcDir, `${locale}.json`), "utf8"));
  let merged = base;
  try {
    const extra = JSON.parse(readFileSync(join(extraDir, `${locale}.json`), "utf8"));
    merged = deepMerge(base, extra);
  } catch {
    console.warn(`[warn] no/invalid extra overlay for ${locale}, using base only`);
  }
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, `${locale}.json`), JSON.stringify(merged, null, 2) + "\n", "utf8");
  report[locale] = new Set(flatKeys(merged));
}

const [zh, en] = LOCALES.map((l) => report[l]);
const onlyZh = [...zh].filter((k) => !en.has(k));
const onlyEn = [...en].filter((k) => !zh.has(k));

console.log(`zh-CN keys: ${zh.size}, en-US keys: ${en.size}`);
console.log(`only in zh-CN (${onlyZh.length}):`, onlyZh);
console.log(`only in en-US (${onlyEn.length}):`, onlyEn);
if (onlyZh.length || onlyEn.length) process.exitCode = 1;
else console.log("key sets are consistent.");
