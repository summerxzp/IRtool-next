// ============================================================
// IRTool Network Attribution Helper — Service Worker (MV3)
// ============================================================
// 监听浏览器网络请求，提取 initiator 字段实现扩展级归因，
// 通过 Native Messaging 批量发送给 IRtool 本地进程。
// ============================================================

"use strict";

// ── 常量 ──────────────────────────────────────────────────────

const NATIVE_HOST = "com.irtool.attribution";

// Ring Buffer 批量发送参数
const BATCH_SIZE = 100;          // 数量阈值：累积 100 条后发送
const BATCH_WINDOW_MS = 500;     // 时间窗口：500ms 后发送

// 心跳间隔
const HEARTBEAT_INTERVAL_MS = 30_000;

// ── 扩展信息缓存 ──────────────────────────────────────────────

/** @type {Map<string, chrome.management.ExtensionInfo>} */
const extensionCache = new Map();

// ── Ring Buffer ───────────────────────────────────────────────

/** @type {object[]} */
let ringBuffer = [];

/** @type {number|null} */
let flushTimer = null;

// ── 过滤配置（由 IRtool 下发） ────────────────────────────────

/** @type {Set<string>|null} 只关注这些域名，null 表示不过滤 */
let filterDomains = null;

// ── Native Messaging 连接状态 ─────────────────────────────────

/** @type {chrome.runtime.Port|null} */
let nativePort = null;

/** @type {number} 连续发送失败计数 */
let sendFailCount = 0;

/// 重连用 chrome.alarms（MV3 service worker 挂起后 setTimeout 会丢失，
/// alarms 不会被取消，适合做持久化重连定时器）
const RECONNECT_ALARM = "irtool-reconnect";

/// 已处理过的 reconnectSignal 时间戳上限。
/// 用于去重：NMH 重启时会重复透传同一份 config（含 reconnectSignal），
/// 没有去重会触发"扩展重连 → NMH 重启 → 又透传 reconnectSignal → 又重连"死循环。
/// 每次收到比这个值大的 reconnectSignal 才响应，并更新此值。
let lastReconnectSignalTs = 0;

/// 自我清理（self-cleanup）alarm 名称。
///
/// 应急响应场景：用户关闭 IRtool 后，扩展应自动卸载，避免给用户设备留痕。
/// 机制：onDisconnect 后启动 alarm，超时后调用 chrome.management.uninstallSelf()。
/// 防误删：alarm 触发时会先试连一次 NMH，连不上才卸载（避免 service worker
/// 挂起导致 alarm 误触发）。
///
/// 超时时间由 IRtool 通过 config.selfCleanupTimeoutMin 下发：
/// - 0 = 禁用自动清理
/// - >0 = 启用，单位分钟（默认 60）
const SELF_CLEANUP_ALARM = "irtool-self-cleanup";
const DEFAULT_SELF_CLEANUP_TIMEOUT_MIN = 60;
let selfCleanupTimeoutMin = DEFAULT_SELF_CLEANUP_TIMEOUT_MIN;

/// 已处理过的 selfUninstall 时间戳上限（去重，机制同 reconnectSignal）
let lastSelfUninstallTs = 0;

/** @type {number|null} setTimeout 重连句柄（≤30s 重连用，需追踪以便取消） */
let reconnectTimer = null;

// ── 心跳定时器 ────────────────────────────────────────────────

/** @type {number|null} */
let heartbeatTimer = null;

// ============================================================
// 工具函数
// ============================================================

/** 生成简易 UUID（crypto.randomUUID 不可用时的回退） */
function uuid() {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === "x" ? r : (r & 0x3) | 0x8).toString(16);
  });
}

/** 从 URL 中提取 hostname */
function hostnameFromUrl(url) {
  try {
    return new URL(url).hostname;
  } catch {
    return "";
  }
}

/** 判断 hostname 是否匹配过滤规则 */
function matchesFilter(hostname) {
  if (filterDomains === null) return true;
  if (filterDomains.size === 0) return true;
  // 精确匹配或父域名匹配
  for (const domain of filterDomains) {
    if (hostname === domain || hostname.endsWith("." + domain)) {
      return true;
    }
  }
  return false;
}

// ============================================================
// Native Messaging
// ============================================================

/**
 * 连接 Native Messaging Host。
 * 使用长连接（connect）而非单次 sendNativeMessage，
 * 以减少反复建立管道的开销。
 */
function connectNative() {
  if (nativePort) return;

  console.log("[IRTool] Connecting to native host:", NATIVE_HOST);

  try {
    nativePort = chrome.runtime.connectNative(NATIVE_HOST);

    nativePort.onMessage.addListener((msg) => {
      handleNativeMessage(msg);
    });

    nativePort.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError;
      console.warn("[IRTool] Native port disconnected:", err?.message || "unknown");
      nativePort = null;
      sendFailCount++;
      scheduleReconnect();
      // 启动自我清理定时器（IRtool 关闭后扩展自动卸载）
      scheduleSelfCleanup();
    });

    // 连接成功，重置失败计数
    sendFailCount = 0;
    console.log("[IRTool] Native messaging connected");
    // IRtool 在线，取消自我清理定时器（防误删）
    cancelSelfCleanup();
  } catch (e) {
    console.warn("[IRTool] Failed to connect native messaging:", e);
    nativePort = null;
    sendFailCount++;
    scheduleReconnect();
    // 首次连接就失败（IRtool 未启动），启动自我清理定时器
    scheduleSelfCleanup();
  }
}

/** 安排重连（指数退避，最大 1 分钟）
 *
 * MV3 service worker 在不活动 30s 后会被挂起，setTimeout 会被取消。
 * 改用 chrome.alarms，它不会被 service worker 挂起影响。
 * alarms 最小间隔约 1 分钟（Chrome 限制），首次重连用 setTimeout（30s 内），
 * 超时后用 alarms 持久化重连。
 */
function scheduleReconnect() {
  // 清除可能存在的旧 alarm
  chrome.alarms.clear(RECONNECT_ALARM);

  // 清除可能存在的旧 setTimeout 重连句柄
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }

  const delayMs = Math.min(1000 * Math.pow(2, sendFailCount), 60_000);

  if (delayMs <= 30_000) {
    // 30s 内用 setTimeout（service worker 通常不会在 30s 内挂起）
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connectNative();
    }, delayMs);
  } else {
    // 超过 30s 用 chrome.alarms（持久化，不受 service worker 挂起影响）
    // alarms 最小 1 分钟，用 delayInMinutes
    chrome.alarms.create(RECONNECT_ALARM, { delayInMinutes: Math.ceil(delayMs / 60_000) });
  }
}

// ============================================================
// 自我清理（Self-Cleanup）
// ============================================================

/**
 * 启动自我清理定时器。
 *
 * 当 NMH 断开（IRtool 关闭）时调用。如果超时时间内没有重连成功，
 * 扩展会调用 chrome.management.uninstallSelf() 自动卸载，避免给用户设备留痕。
 *
 * 防误删：alarm 触发时会先试连一次 NMH，连不上才卸载。
 * 超时时间 selfCleanupTimeoutMin：
 * - 0 = 禁用自动清理（不创建 alarm）
 * - >0 = 启用，单位分钟
 */
function scheduleSelfCleanup() {
  // 先清旧的 alarm（避免叠加）
  chrome.alarms.clear(SELF_CLEANUP_ALARM);

  // 0 = 禁用
  if (selfCleanupTimeoutMin <= 0) {
    console.log("[IRTool] Self-cleanup disabled (timeout=0)");
    return;
  }

  console.log(
    "[IRTool] Scheduling self-cleanup in",
    selfCleanupTimeoutMin,
    "minutes (IRtool offline)"
  );
  chrome.alarms.create(SELF_CLEANUP_ALARM, {
    delayInMinutes: selfCleanupTimeoutMin,
  });
}

/** 取消自我清理定时器（IRtool 在线时调用，防误删） */
function cancelSelfCleanup() {
  chrome.alarms.clear(SELF_CLEANUP_ALARM);
}

/**
 * 自我清理 alarm 触发时的处理。
 *
 * 防误删机制：先试连一次 NMH，连得上说明 IRtool 刚回来，取消清理；
 * 连不上才真正调用 uninstallSelf。
 *
 * 场景：用户临时关 IRtool 几分钟再开，alarm 可能在 IRtool 回来后立即触发，
 * 这时试连能成功，避免误删。
 */
function onSelfCleanupAlarmFired() {
  console.log("[IRTool] Self-cleanup alarm fired, verifying IRtool is still offline");

  // 防误删：先试连一次
  try {
    const testPort = chrome.runtime.connectNative(NATIVE_HOST);
    testPort.onDisconnect.addListener(() => {
      // 连不上（NMH 不存在或立即断开）→ 真正卸载
      console.log("[IRTool] IRtool confirmed offline, uninstalling self");
      chrome.management.uninstallSelf(
        { showConfirmDialog: false },
        () => {
          if (chrome.runtime.lastError) {
            console.warn(
              "[IRTool] uninstallSelf failed:",
              chrome.runtime.lastError.message
            );
          }
        }
      );
    });
    testPort.onMessage.addListener(() => {
      // 收到消息说明 NMH 在线 → 取消清理
      console.log("[IRTool] IRtool is back online, canceling self-cleanup");
      try { testPort.disconnect(); } catch (e) {}
      cancelSelfCleanup();
      // 顺便恢复主连接
      connectNative();
    });
    // 1 秒内没收到消息也没断开？保守起见也取消清理（可能 NMH 在但没消息）
    setTimeout(() => {
      try { testPort.disconnect(); } catch (e) {}
    }, 1000);
  } catch (e) {
    // connectNative 抛异常 → NMH 不存在 → 卸载
    console.log("[IRTool] NMH not reachable, uninstalling self");
    chrome.management.uninstallSelf(
      { showConfirmDialog: false },
      () => {
        if (chrome.runtime.lastError) {
          console.warn(
            "[IRTool] uninstallSelf failed:",
            chrome.runtime.lastError.message
          );
        }
      }
    );
  }
}

/**
 * 通过 Native Messaging 发送消息。
 * 如果长连接不可用，回退到 sendNativeMessage 单次发送。
 *
 * 返回值：
 * - true：消息已成功投递（长连接 postMessage 成功，或单次发送已发起）
 * - false：消息发送失败（nativePort 不存在 + 单次发送也失败）
 *
 * 注意：单次发送（sendNativeMessage）是异步的，返回 true 只表示调用已发起，
 * 不保证 NMH 端收到。但失败会通过 sendFailCount 跟踪，触发重连。
 * 对于 flush 调用方，如果返回 false 需要把事件放回 ringBuffer 重试。
 */
function sendNativeMessage(message) {
  // 优先使用长连接
  if (nativePort) {
    try {
      nativePort.postMessage(message);
      sendFailCount = 0;
      return true;
    } catch (e) {
      console.warn("[IRTool] Long-connection send failed, falling back:", e);
      nativePort = null;
      scheduleReconnect();
    }
  }

  // 回退：单次发送
  try {
    chrome.runtime.sendNativeMessage(NATIVE_HOST, message, () => {
      if (chrome.runtime.lastError) {
        sendFailCount++;
        console.warn("[IRTool] sendNativeMessage failed:", chrome.runtime.lastError.message);
      } else {
        sendFailCount = 0;
      }
    });
    return true; // 单次发送已发起（异步回调跟踪结果）
  } catch (e) {
    sendFailCount++;
    console.warn("[IRTool] sendNativeMessage threw:", e);
    return false;
  }
}

/** 处理来自 IRtool 的消息 */
function handleNativeMessage(msg) {
  if (!msg || typeof msg !== "object") return;

  switch (msg.type) {
    case "config":
      applyConfig(msg);
      break;
    default:
      console.log("[IRTool] Unknown native message type:", msg.type);
  }
}

/** 应用 IRtool 下发的配置 */
function applyConfig(msg) {
  // 处理重连信号（用户点击 IRtool 的"重新连接"按钮触发）
  // 收到后立即重置退避计数器并尝试重连，省去等待指数退避的时间
  //
  // 去重：reconnectSignal 是带时间戳的一次性事件，但 NMH 重启时会重新透传
  // 整份 config（含 reconnectSignal 字段），不去重会触发
  // "扩展重连 → NMH 重启 → 又透传 → 又重连" 死循环。
  // 只响应当前时间戳严格大于 lastReconnectSignalTs 的信号。
  if (msg.reconnectSignal && typeof msg.reconnectSignal === "number") {
    if (msg.reconnectSignal <= lastReconnectSignalTs) {
      console.log(
        "[IRTool] Stale reconnect signal ignored:",
        msg.reconnectSignal,
        "(last processed:",
        lastReconnectSignalTs,
        ")"
      );
      // 注意：不走 return，继续往下处理 filterDomains（reconnectSignal 与
      // filterDomains 可能在同一条 config 消息中，不能因去重跳过 filter 更新）
    } else {
      lastReconnectSignalTs = msg.reconnectSignal;
      console.log("[IRTool] Reconnect signal received, resetting backoff and reconnecting now");
      sendFailCount = 0;
      chrome.alarms.clear(RECONNECT_ALARM);
      if (reconnectTimer) {
        clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (nativePort) {
        try { nativePort.disconnect(); } catch (e) {}
        nativePort = null;
      }
      if (flushTimer) {
        clearTimeout(flushTimer);
        flushTimer = null;
      }
      connectNative();
      // 注意：不走 return，继续往下处理 filterDomains
    }
  }

  if (msg.filterDomains && Array.isArray(msg.filterDomains)) {
    filterDomains = new Set(msg.filterDomains);
    console.log("[IRTool] Filter applied:", msg.filterDomains.length, "domains");
  } else if (msg.filterDomains === null || msg.filterDomains === false) {
    filterDomains = null;
    console.log("[IRTool] Filter cleared");
  }

  // 处理自我清理超时配置（IRtool 下发，0=禁用，>0=启用）
  if (typeof msg.selfCleanupTimeoutMin === "number") {
    const newTimeout = Math.max(0, Math.floor(msg.selfCleanupTimeoutMin));
    if (newTimeout !== selfCleanupTimeoutMin) {
      selfCleanupTimeoutMin = newTimeout;
      console.log(
        "[IRTool] Self-cleanup timeout updated:",
        newTimeout === 0 ? "disabled" : `${newTimeout} minutes`
      );
      // 如果当前已断开（已 schedule 清理），重新 schedule 用新超时
      if (!nativePort) {
        scheduleSelfCleanup();
      }
    }
  }

  // 处理手动 selfUninstall 信号（用户在 IRtool UI 点击"清理扩展"触发）
  // 带时间戳去重，机制同 reconnectSignal
  if (msg.selfUninstall && typeof msg.selfUninstall === "number") {
    if (msg.selfUninstall <= lastSelfUninstallTs) {
      console.log(
        "[IRTool] Stale selfUninstall signal ignored:",
        msg.selfUninstall,
        "(last processed:",
        lastSelfUninstallTs,
        ")"
      );
    } else {
      lastSelfUninstallTs = msg.selfUninstall;
      console.log("[IRTool] Self-uninstall signal received, uninstalling now");
      // 立即卸载，不弹确认对话框
      chrome.management.uninstallSelf(
        { showConfirmDialog: false },
        () => {
          if (chrome.runtime.lastError) {
            console.warn(
              "[IRTool] uninstallSelf failed:",
              chrome.runtime.lastError.message
            );
          }
        }
      );
    }
  }
}

// ============================================================
// Ring Buffer 批量发送
// ============================================================

/** 将一个归因事件加入 Ring Buffer */
function enqueue(event) {
  ringBuffer.push(event);

  // 数量阈值触发
  if (ringBuffer.length >= BATCH_SIZE) {
    flush();
    return;
  }

  // 时间窗口触发：首个事件入队时启动定时器
  if (!flushTimer) {
    flushTimer = setTimeout(() => {
      flushTimer = null;
      flush();
    }, BATCH_WINDOW_MS);
  }
}

/** 将 Ring Buffer 中的事件批量发送。失败时把事件放回 ringBuffer 等待重试。 */
function flush() {
  if (flushTimer) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }

  if (ringBuffer.length === 0) return;

  const batch = ringBuffer;
  ringBuffer = [];

  const message = {
    type: "network_batch",
    batch_id: uuid(),
    events: batch,
  };

  const ok = sendNativeMessage(message);
  if (!ok) {
    // 发送失败：把事件放回 ringBuffer 前面，下次重试
    // 防止事件丢失（应急响应场景要求所有请求都要抓到）
    ringBuffer = batch.concat(ringBuffer);
    console.warn(
      "[IRTool] flush failed,",
      batch.length,
      "events returned to buffer (total:",
      ringBuffer.length,
      "), will retry"
    );
    // 限制 ringBuffer 大小，防止无限增长（极端情况下最多保留 1000 条）
    if (ringBuffer.length > 1000) {
      console.warn("[IRTool] ringBuffer overflow, dropping", ringBuffer.length - 1000, "old events");
      ringBuffer = ringBuffer.slice(-1000);
    }
    // 1 秒后重试（用 setTimeout，如果 service worker 挂起会被取消，
    // 但下次 onBeforeRequest 触发时会重新启动 flush 定时器）
    if (!flushTimer) {
      flushTimer = setTimeout(() => {
        flushTimer = null;
        flush();
      }, 1000);
    }
  } else {
    console.log("[IRTool] Flushed", batch.length, "events");
  }
}

// ============================================================
// 网络请求归因
// ============================================================

/**
 * webRequest.onBeforeRequest 回调。
 * 提取 initiator 字段，判断归因类型，加入 Ring Buffer。
 */
function onBeforeRequest(details) {
  const { requestId, url, method, initiator } = details;

  // 过滤：仅关注 http/https 请求
  if (!url.startsWith("http")) return;

  // 过滤：域名过滤
  const hostname = hostnameFromUrl(url);
  if (!matchesFilter(hostname)) return;

  // 调试日志：确认 onBeforeRequest 触发了哪些请求（帮助定位 404/失败请求是否被抓取）
  console.log(
    "[IRTool] onBeforeRequest:",
    method,
    url,
    "| hostname:",
    hostname,
    "| initiator:",
    initiator || "(none)",
    "| nativePort:",
    nativePort ? "connected" : "disconnected"
  );

  // MV3 service worker 唤醒后 nativePort 可能丢失，检查并重连
  if (!nativePort) {
    connectNative();
  }

  let attribution;

  if (initiator && initiator.startsWith("chrome-extension://")) {
    // 从 initiator 中提取 extension ID
    // 格式：chrome-extension://<extension-id>/
    const match = initiator.match(/^chrome-extension:\/\/([a-z]{32})/);
    const extensionId = match ? match[1] : null;

    if (extensionId) {
      const extInfo = extensionCache.get(extensionId);
      attribution = {
        status: "high-confidence",
        extensionId,
        extensionName: extInfo ? extInfo.name : "Unknown",
      };
    } else {
      attribution = {
        status: "high-confidence",
        extensionId: null,
        extensionName: "Unknown",
      };
    }
  } else if (initiator && initiator.startsWith("http")) {
    // 页面 origin 发起的请求
    attribution = {
      status: "page-originated",
      extensionId: null,
      extensionName: null,
    };
  } else {
    // 无 initiator 或其他情况
    attribution = {
      status: "browser-owned",
      extensionId: null,
      extensionName: null,
    };
  }

  enqueue({
    timestamp: Date.now(),
    requestId,
    url,
    method,
    initiator: initiator || null,
    attribution,
  });
}

// ============================================================
// 扩展清单上报
// ============================================================

/** 全量上报所有扩展 */
async function reportExtensionListFull() {
  try {
    const extensions = await chrome.management.getAll();
    // 更新缓存
    extensionCache.clear();
    for (const ext of extensions) {
      extensionCache.set(ext.id, ext);
    }

    const message = {
      type: "extension_list",
      mode: "full",
      extensions: extensions.map((ext) => ({
        id: ext.id,
        name: ext.name,
        version: ext.version,
        enabled: ext.enabled,
        hostPermissions: ext.hostPermissions || [],
        installType: ext.installType,
      })),
    };

    sendNativeMessage(message);
    console.log("[IRTool] Extension list reported:", extensions.length, "extensions");
  } catch (e) {
    console.warn("[IRTool] Failed to get extension list:", e);
  }
}

/** 增量上报：扩展安装 */
function onExtensionInstalled(ext) {
  extensionCache.set(ext.id, ext);

  const message = {
    type: "extension_list",
    mode: "incremental",
    extensions: [
      {
        id: ext.id,
        name: ext.name,
        version: ext.version,
        enabled: ext.enabled,
        hostPermissions: ext.hostPermissions || [],
        installType: ext.installType,
      },
    ],
  };

  sendNativeMessage(message);
}

/** 增量上报：扩展卸载 */
function onExtensionUninstalled(id) {
  extensionCache.delete(id);

  const message = {
    type: "extension_list",
    mode: "incremental",
    extensions: [
      {
        id,
        name: null,
        version: null,
        enabled: false,
        hostPermissions: [],
        installType: "uninstalled",
      },
    ],
  };

  sendNativeMessage(message);
}

// ============================================================
// 心跳
// ============================================================

function startHeartbeat() {
  if (heartbeatTimer) return;
  heartbeatTimer = setInterval(() => {
    sendNativeMessage({
      type: "heartbeat",
      timestamp: Date.now(),
    });
  }, HEARTBEAT_INTERVAL_MS);
}

// ============================================================
// 初始化
// ============================================================

// MV3 service worker 生命周期：
// - 首次安装/更新/启用时：脚本完整执行一次
// - Chrome 重启后：service worker 不会自动启动，只在事件触发时唤醒
// - 唤醒时：脚本完整重新执行（全局变量重置）
//
// 因此所有事件监听器必须在顶层注册（不能放在 if 块内），
// connectNative() 在每次脚本执行时调用。

function init() {
  console.log("[IRTool] Service Worker starting (init)");

  // 1. 连接 Native Messaging Host
  connectNative();

  // 2. 监听网络请求
  chrome.webRequest.onBeforeRequest.addListener(
    onBeforeRequest,
    { urls: ["<all_urls>"] },
    []
  );

  // 3. 监听扩展安装/卸载
  chrome.management.onInstalled.addListener(onExtensionInstalled);
  chrome.management.onUninstalled.addListener(onExtensionUninstalled);

  // 4. 全量上报扩展清单
  reportExtensionListFull();

  // 5. 启动心跳
  startHeartbeat();

  // 6. 监听 alarms（用于 service worker 挂起后的持久化重连 + 自我清理）
  chrome.alarms.onAlarm.addListener((alarm) => {
    if (alarm.name === RECONNECT_ALARM) {
      console.log("[IRTool] Reconnect alarm fired, attempting reconnect");
      connectNative();
    } else if (alarm.name === SELF_CLEANUP_ALARM) {
      onSelfCleanupAlarmFired();
    }
  });

  console.log("[IRTool] Service Worker initialized");
}

// 启动
init();
