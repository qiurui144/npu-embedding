/**
 * Background Service Worker — 消息路由 + API 调度 + 去重
 */

import { API } from '../shared/api.js';
import { getSettings, saveSettings } from '../shared/storage.js';
import { MSG, sendToTab } from '../shared/messages.js';

const api = new API();

// --- 去重缓存 (hash -> timestamp) ---
const dedup = new Map();
const DEDUP_TTL = 60 * 60 * 1000; // 1h

// --- 连接状态 ---
let backendOnline = false;
let injectionEnabled = true;

// --- 初始化 ---
chrome.runtime.onInstalled.addListener(() => {
  console.log('[npu-webhook] Extension installed');
  initState();
});

chrome.runtime.onStartup?.addListener(() => {
  initState();
});

async function initState() {
  await api.reloadBaseUrl();
  // 恢复去重缓存
  try {
    const session = await chrome.storage.session.get('dedup');
    if (session.dedup) {
      for (const [k, v] of Object.entries(session.dedup)) {
        dedup.set(k, v);
      }
    }
  } catch { /* session storage 可能不可用 */ }
  // 恢复注入状态
  try {
    const result = await chrome.storage.local.get('injectionEnabled');
    if (result.injectionEnabled !== undefined) injectionEnabled = result.injectionEnabled;
  } catch { /* */ }
  healthCheck();
}

// --- 消息路由 ---
chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  handleMessage(msg, sender).then(sendResponse).catch((err) => {
    console.error('[npu-webhook] Message handler error:', err);
    sendResponse({ error: err.message });
  });
  return true; // 异步 sendResponse
});

async function handleMessage(msg, sender) {
  switch (msg.type) {
    case MSG.CAPTURE_CONVERSATION:
      return handleCapture(msg.data);

    case MSG.SEARCH_RELEVANT:
      return api.searchRelevant({ query: msg.query, top_k: msg.top_k || 3 });

    case MSG.GET_STATUS: {
      let status = { online: backendOnline, injection_enabled: injectionEnabled };
      if (backendOnline) {
        try {
          const s = await api.status();
          status = { ...status, ...s };
        } catch { /* */ }
      }
      return status;
    }

    case MSG.SEARCH:
      return api.search(msg.query, { topK: msg.top_k, sourceTypes: msg.source_types });

    case MSG.GET_ITEMS:
      return api.items({ offset: msg.offset, limit: msg.limit, sourceType: msg.source_type });

    case MSG.GET_SETTINGS:
      return getSettings();

    case MSG.TOGGLE_INJECTION: {
      injectionEnabled = msg.enabled;
      await chrome.storage.local.set({ injectionEnabled });
      // 通知所有 content script tabs
      const tabs = await chrome.tabs.query({});
      for (const tab of tabs) {
        try {
          await sendToTab(tab.id, MSG.TOGGLE_INJECTION, { enabled: injectionEnabled });
        } catch { /* tab 可能没有 content script */ }
      }
      return { ok: true };
    }

    case MSG.OPEN_SIDEPANEL:
      if (sender.tab) {
        await chrome.sidePanel.open({ tabId: sender.tab.id });
      }
      return { ok: true };

    default:
      return { error: `Unknown message type: ${msg.type}` };
  }
}

// --- 对话捕获去重 + 入库 ---
function djb2(str) {
  let h = 5381;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) + h + str.charCodeAt(i)) >>> 0;
  }
  return h.toString(36);
}

async function handleCapture(data) {
  if (!data?.content) return { error: 'No content' };

  const hash = djb2(data.content);
  if (dedup.has(hash)) return { status: 'duplicate' };

  dedup.set(hash, Date.now());
  persistDedup();

  try {
    const result = await api.ingest(data);
    return { status: 'ok', id: result.id };
  } catch (err) {
    // 入库失败，从去重缓存移除以便重试
    dedup.delete(hash);
    throw err;
  }
}

function persistDedup() {
  const obj = Object.fromEntries(dedup);
  chrome.storage.session.set({ dedup: obj }).catch(() => {});
}

// --- 定期健康检查 + 缓存清理 ---
async function healthCheck() {
  try {
    await api.reloadBaseUrl();
    await api.health();
    backendOnline = true;
  } catch {
    backendOnline = false;
  }

  // 清理过期去重缓存
  const now = Date.now();
  for (const [k, ts] of dedup) {
    if (now - ts > DEDUP_TTL) dedup.delete(k);
  }
  if (dedup.size > 0) persistDedup();

  setTimeout(healthCheck, 30000);
}

// 首次运行
initState();
