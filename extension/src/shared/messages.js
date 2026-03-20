/**
 * 统一消息类型 + 通信辅助
 */

export const MSG = {
  CAPTURE_CONVERSATION: 'CAPTURE_CONVERSATION',
  SEARCH_RELEVANT: 'SEARCH_RELEVANT',
  GET_STATUS: 'GET_STATUS',
  TOGGLE_INJECTION: 'TOGGLE_INJECTION',
  SEARCH: 'SEARCH',
  GET_ITEMS: 'GET_ITEMS',
  GET_SETTINGS: 'GET_SETTINGS',
  OPEN_SIDEPANEL: 'OPEN_SIDEPANEL',
  SETTINGS_UPDATED: 'SETTINGS_UPDATED',
  // Phase 2.5 新增
  SAVE_SELECTION: 'SAVE_SELECTION',      // 右键选中文本入库
  SUMMARIZE_AND_SAVE: 'SUMMARIZE_AND_SAVE', // 对话摘要后入库
  // Phase 3 新增
  PREFETCH: 'PREFETCH',                  // 打字时预取知识（减少注入延迟）
  FILE_UPLOADED: 'FILE_UPLOADED',        // 文件上传成功，记录会话 ID 用于搜索加权
};

/** 发消息到 Background Worker */
export function sendToWorker(type, payload = {}) {
  return chrome.runtime.sendMessage({ type, ...payload });
}

/** 从 Worker 发消息到指定 Tab */
export function sendToTab(tabId, type, payload = {}) {
  return chrome.tabs.sendMessage(tabId, { type, ...payload });
}
