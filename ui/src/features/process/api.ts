import { commands } from "@/lib/bindings";

export async function getSnapshot() {
  const result = await commands.cmdProcessSnapshot();
  if (result.status === "error") throw result.error;
  return result.data;
}

export async function getProcessChain(pid: number) {
  const result = await commands.cmdProcessChain(pid);
  if (result.status === "error") throw result.error;
  return result.data;
}
