/**
 * 后端 API 封装
 */

const DEFAULT_BASE_URL = 'http://localhost:18900/api/v1';

export class API {
  constructor(baseUrl = DEFAULT_BASE_URL) {
    this.baseUrl = baseUrl;
  }

  /** 从 chrome.storage 重新加载 baseUrl */
  async reloadBaseUrl() {
    try {
      const result = await chrome.storage.local.get('settings');
      const settings = result.settings;
      if (settings?.backendUrl) {
        this.baseUrl = settings.backendUrl.replace(/\/+$/, '') + '/api/v1';
      }
    } catch { /* content script 环境可能无权限 */ }
  }

  async request(path, options = {}) {
    const resp = await fetch(`${this.baseUrl}${path}`, {
      headers: { 'Content-Type': 'application/json', ...options.headers },
      ...options,
    });
    if (!resp.ok) throw new Error(`API error: ${resp.status}`);
    return resp.json();
  }

  // --- 基础 ---
  health() { return this.request('/status/health'); }
  status() { return this.request('/status'); }

  // --- 知识注入 ---
  ingest(data) { return this.request('/ingest', { method: 'POST', body: JSON.stringify(data) }); }

  // --- 搜索 ---
  search(query, { topK = 10, sourceTypes } = {}) {
    let url = `/search?q=${encodeURIComponent(query)}&top_k=${topK}`;
    if (sourceTypes) url += `&source_types=${encodeURIComponent(sourceTypes)}`;
    return this.request(url);
  }

  searchRelevant(data) {
    return this.request('/search/relevant', { method: 'POST', body: JSON.stringify(data) });
  }

  // --- 条目 CRUD ---
  items({ offset = 0, limit = 20, sourceType } = {}) {
    let url = `/items?offset=${offset}&limit=${limit}`;
    if (sourceType) url += `&source_type=${encodeURIComponent(sourceType)}`;
    return this.request(url);
  }

  getItem(id) { return this.request(`/items/${id}`); }

  deleteItem(id) { return this.request(`/items/${id}`, { method: 'DELETE' }); }

  updateItem(id, data) {
    return this.request(`/items/${id}`, { method: 'PATCH', body: JSON.stringify(data) });
  }

  // --- 设置 ---
  getSettings() { return this.request('/settings'); }

  updateSettings(data) {
    return this.request('/settings', { method: 'PATCH', body: JSON.stringify(data) });
  }

  // --- 索引 ---
  indexStatus() { return this.request('/index/status'); }

  /** 上传原始文件到 /upload（multipart/form-data）*/
  async uploadFile(file, sessionId = null) {
    const form = new FormData();
    form.append('file', file);
    if (sessionId) form.append('session_id', sessionId);
    const resp = await fetch(`${this.baseUrl}/upload`, {
      method: 'POST',
      body: form,
      // 不设 Content-Type，让浏览器自动设置 multipart boundary
    });
    if (!resp.ok) throw new Error(`Upload error: ${resp.status}`);
    return resp.json();
  }
}

export const api = new API();
