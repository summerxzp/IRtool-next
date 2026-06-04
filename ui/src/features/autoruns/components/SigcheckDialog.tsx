import { useAutorunsStore } from "../store";

export function SigcheckDialog() {
  const sigcheckResult = useAutorunsStore((s) => s.sigcheckResult);
  const setSigcheckResult = useAutorunsStore((s) => s.setSigcheckResult);

  if (!sigcheckResult) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={() => setSigcheckResult(null)}>
      <div className="bg-bg-elev-1 border border-border rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[80vh] flex flex-col" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <span className="text-sm font-medium">Sigcheck 验证结果</span>
          <button className="text-fg-tertiary hover:text-fg-primary" onClick={() => setSigcheckResult(null)}>✕</button>
        </div>
        <div className="px-4 py-2 text-xs text-fg-tertiary font-mono break-all border-b border-border">
          {sigcheckResult.path}
        </div>
        <pre className="flex-1 overflow-auto p-4 text-xs font-mono whitespace-pre-wrap text-fg-primary">
          {sigcheckResult.output}
        </pre>
      </div>
    </div>
  );
}
