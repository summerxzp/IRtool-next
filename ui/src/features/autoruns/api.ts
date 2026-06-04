import { commands } from "@/lib/bindings";
import type { ScanOptions } from "./types";

export async function scan(options: ScanOptions) {
  const result = await commands.cmdAutorunsScan(options);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function getResult() {
  const result = await commands.cmdAutorunsGetResult();
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function verifySignatures(paths: string[]) {
  const result = await commands.cmdAutorunsVerifySignatures(paths);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function deleteEntry(entryId: number) {
  const result = await commands.cmdAutorunsDeleteEntry(entryId);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function cancelScan(taskId: number) {
  const result = await commands.cmdAutorunsCancelScan(taskId);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function calculateHash(entryId: number) {
  const result = await commands.cmdAutorunsCalculateHash(entryId);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function sigcheck(imagePath: string) {
  const result = await commands.cmdAutorunsSigcheck(imagePath);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function openExplorer(path: string) {
  const result = await commands.cmdAutorunsOpenExplorer(path);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function openRegedit(registryPath: string) {
  const result = await commands.cmdAutorunsOpenRegedit(registryPath);
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function openServices() {
  const result = await commands.cmdAutorunsOpenServices();
  if (result.status === "error") throw result.error;
  return result.data;
}
