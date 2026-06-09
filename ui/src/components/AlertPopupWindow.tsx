import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

type PopupData = {
  rule_name: string;
  key_field: string;
  event_type?: string;
  source_addr?: string;
  remote_addr?: string;
  protocol?: string;
  process_chain?: string;
  action_taken?: string;
  timestamp: string;
  duration_secs?: number;
};

export default function AlertPopupWindow() {
  const [popup, setPopup] = useState<PopupData | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    let dismissTimer: ReturnType<typeof setTimeout> | null = null;
    let animFrame: number | null = null;

    const unlisten = listen<PopupData>("evt_show_alert_popup", (event) => {
      setPopup(event.payload);
      if (animFrame) cancelAnimationFrame(animFrame);
      animFrame = requestAnimationFrame(() => {
        setVisible(true);
        animFrame = null;
      });
      // Show the window now that content is ready — avoids white flash
      getCurrentWindow().show().catch(() => {});
      if (dismissTimer) clearTimeout(dismissTimer);
      const duration = (event.payload.duration_secs ?? 10) * 1000;
      if (duration > 0) {
        dismissTimer = setTimeout(() => {
          getCurrentWindow().close().catch(() => {});
        }, duration);
      }
    });

    const unlistenDismiss = listen("evt_dismiss_alert_popup", () => {
      if (dismissTimer) clearTimeout(dismissTimer);
      getCurrentWindow().close().catch(() => {});
    });

    return () => {
      unlisten.then((fn) => fn());
      unlistenDismiss.then((fn) => fn());
      if (dismissTimer) clearTimeout(dismissTimer);
      if (animFrame) cancelAnimationFrame(animFrame);
    };
  }, []);

  const handleClick = () => {
    if (popup) {
      import("@tauri-apps/api/event").then(({ emit }) => {
        emit("evt_alert_popup_clicked", {
          alert_key: popup.key_field,
          event_type: (popup as Record<string, unknown>).event_type as string | undefined,
        }).catch(() => {});
      });
      getCurrentWindow().close().catch(() => {});
    }
  };

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation();
    getCurrentWindow().close().catch(() => {});
  };

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        background: "var(--bg-elev-1)",
        borderLeft: "3px solid var(--danger)",
        borderRadius: "6px",
        boxShadow: "0 4px 24px rgba(0,0,0,0.25)",
        cursor: "pointer",
        display: "flex",
        flexDirection: "column",
        opacity: visible ? 1 : 0,
        transform: visible ? "translateY(0)" : "translateY(-8px)",
        transition: "opacity 300ms cubic-bezier(0.16, 1, 0.3, 1), transform 300ms cubic-bezier(0.16, 1, 0.3, 1)",
      }}
      onClick={handleClick}
    >
      {!popup ? null : (
        <>
          {/* Header */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "8px 10px",
              background: "var(--bg-elev-2)",
              flexShrink: 0,
            }}
          >
            <div
              style={{
                width: "18px",
                height: "18px",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                flexShrink: 0,
              }}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="var(--danger)"
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
            </div>
            <span style={{ color: "var(--danger)", fontSize: "12px", fontWeight: 600, flex: 1 }}>
              安全告警
            </span>
            <span style={{ fontSize: "10px", color: "var(--fg-tertiary)" }}>{popup.timestamp}</span>
            <button
              onClick={handleClose}
              style={{
                background: "none",
                border: "none",
                color: "var(--fg-tertiary)",
                cursor: "pointer",
                padding: "0 2px",
                fontSize: "14px",
                lineHeight: 1,
              }}
            >
              ×
            </button>
          </div>

          {/* Content */}
          <div style={{ padding: "8px 10px", flex: 1, display: "flex", flexDirection: "column", gap: "4px" }}>
            <div style={{ fontSize: "13px", fontWeight: 600, color: "var(--fg-primary)" }}>
              {popup.rule_name}
            </div>
            <div
              style={{
                fontSize: "12px",
                color: "var(--fg-secondary)",
                fontFamily: '"SF Mono", "Cascadia Code", "Consolas", monospace',
                wordBreak: "break-all",
              }}
            >
              {popup.key_field}
            </div>
            {popup.source_addr && (
              <div style={{ fontSize: "11px", color: "var(--fg-tertiary)" }}>
                源地址: {popup.source_addr}
              </div>
            )}
            {popup.remote_addr && (
              <div style={{ fontSize: "11px", color: "var(--fg-tertiary)" }}>
                目标地址: {popup.remote_addr}
              </div>
            )}
            {popup.protocol && (
              <div style={{ fontSize: "11px", color: "var(--fg-tertiary)" }}>
                协议: {popup.protocol}
              </div>
            )}
            {popup.process_chain && (
              <div
                style={{
                  fontSize: "11px",
                  color: "var(--fg-tertiary)",
                  fontFamily: '"SF Mono", "Cascadia Code", "Consolas", monospace',
                  wordBreak: "break-all",
                  marginTop: "2px",
                }}
              >
                进程链条: {popup.process_chain}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
