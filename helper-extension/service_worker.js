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

/** @type {number|null} 重连定时器 */
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

  try {
    nativePort = chrome.runtime.connectNative(NATIVE_HOST);

    nativePort.onMessage.addListener((msg) => {
      handleNativeMessage(msg);
    });

    nativePort.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError;
      console.warn("[IRTool] Native port disconnected:", err?.message || "unknown");
      nativePort = null;
      scheduleReconnect();
    });

    // 连接成功，重置失败计数
    sendFailCount = 0;
    console.log("[IRTool] Native messaging connected");
  } catch (e) {
    console.warn("[IRTool] Failed to connect native messaging:", e);
    nativePort = null;
    scheduleReconnect();
  }
}

/** 安排重连（指数退避，最大 30s） */
function scheduleReconnect() {
  if (reconnectTimer) return;
  const delay = Math.min(1000 * Math.pow(2, sendFailCount), 30_000);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectNative();
  }, delay);
}

/**
 * 通过 Native Messaging 发送消息。
 * 如果长连接不可用，回退到 sendNativeMessage 单次发送。
 */
function sendNativeMessage(message) {
  // 优先使用长连接
  if (nativePort) {
    try {
      nativePort.postMessage(message);
      sendFailCount = 0;
      return;
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
  } catch (e) {
    sendFailCount++;
    console.warn("[IRTool] sendNativeMessage threw:", e);
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
  if (msg.filterDomains && Array.isArray(msg.filterDomains)) {
    filterDomains = new Set(msg.filterDomains);
    console.log("[IRTool] Filter applied:", msg.filterDomains.length, "domains");
  } else if (msg.filterDomains === null || msg.filterDomains === false) {
    filterDomains = null;
    console.log("[IRTool] Filter cleared");
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

/** 将 Ring Buffer 中的事件批量发送 */
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

  sendNativeMessage(message);
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

function init() {
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

  console.log("[IRTool] Service Worker initialized");
}

// 启动
init();
