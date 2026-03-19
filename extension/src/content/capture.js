/**
 * 对话捕获 — MutationObserver 监听 AI 回答，配对后推送摘要到 Worker
 *
 * 修复：
 * - WeakMap 替代 Map 存储 per-node timer，避免 DOM 节点内存泄漏
 * - 队列替代单变量存储 pendingUser，支持多轮追问正确配对
 * - sendToWorker 失败时从 _seen 移除 hash，允许下次重试
 * - _markExisting() 仅扫描最近 50 条消息，避免大聊天记录初始化卡顿
 */

import { MSG, sendToWorker } from '../shared/messages.js';

const MAX_EXISTING_SCAN = 50; // 初始化时最多扫描的历史消息数量

/** djb2 hash */
function hashStr(str) {
  let h = 5381;
  for (let i = 0; i < str.length; i++) {
    h = ((h << 5) + h + str.charCodeAt(i)) >>> 0;
  }
  return h.toString(36);
}

export class ConversationCapture {
  constructor(adapter) {
    this.adapter = adapter;
    this._observer = null;
    this._seen = new Set();
    this._pendingUsers = []; // 队列支持多轮追问
    this._nodeTimers = new WeakMap(); // WeakMap 避免 DOM 节点内存泄漏
    this._retryTimers = []; // 存储重试 timer 以便 stop() 时清理
  }

  start() {
    const container = document.querySelector(this.adapter.messagesContainer);
    if (!container) {
      setTimeout(() => this.start(), 2000);
      return;
    }

    this._markExisting();

    this._observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === 'childList') {
          for (const node of mutation.addedNodes) {
            if (node.nodeType === Node.ELEMENT_NODE) {
              this._handleNode(node);
            }
          }
        }
      }
    });

    this._observer.observe(container, { childList: true, subtree: true });
    console.log('[npu-webhook] Capture started');
  }

  stop() {
    if (this._observer) {
      this._observer.disconnect();
      this._observer = null;
    }
    // WeakMap 不可迭代，retryTimers 单独清理
    for (const t of this._retryTimers) clearTimeout(t);
    this._retryTimers = [];
  }

  /** 只扫描最近 N 条消息，避免大聊天记录初始化卡顿 */
  _markExisting() {
    const nodes = document.querySelectorAll(this.adapter.messages);
    const recent = Array.from(nodes).slice(-MAX_EXISTING_SCAN);
    for (const node of recent) {
      const msg = this.adapter.extractMessage(node);
      if (msg?.content) this._seen.add(hashStr(msg.content));
    }
  }

  _handleNode(node) {
    const msgNode = node.matches?.(this.adapter.messages) ? node : node.querySelector?.(this.adapter.messages);
    if (!msgNode) return;

    // per-node debounce: WeakMap key 为 DOM 节点，节点被 GC 后自动释放
    const existing = this._nodeTimers.get(msgNode);
    if (existing) clearTimeout(existing);
    this._nodeTimers.set(msgNode, setTimeout(() => {
      this._processNode(msgNode);
    }, 2000));
  }

  _processNode(node) {
    if (!this.adapter.isComplete(node)) {
      const t = setTimeout(() => this._processNode(node), 1000);
      this._retryTimers.push(t);
      return;
    }

    const msg = this.adapter.extractMessage(node);
    if (!msg?.content) return;

    const h = hashStr(msg.content);
    if (this._seen.has(h)) return;
    this._seen.add(h);

    if (msg.role === 'user') {
      // 队列存储，支持连续问题的正确配对
      this._pendingUsers.push(msg);
    } else if (msg.role === 'assistant' && this._pendingUsers.length > 0) {
      const user = this._pendingUsers.shift();
      this._sendPair(user, msg);
    }
  }

  _sendPair(user, assistant) {
    const title = user.content.slice(0, 100);
    const fullContent = `用户: ${user.content}\n\n助手: ${assistant.content}`;

    if (fullContent.length < 500) {
      this._directSave(title, fullContent);
    } else {
      this._summarizeAndSave(title, fullContent);
    }
  }

  /** 短对话直接入库；失败时从 _seen 移除 hash 允许重试 */
  _directSave(title, content) {
    const hash = hashStr(content);
    const data = {
      title,
      content,
      source_type: 'ai_chat',
      url: location.href,
      domain: location.hostname,
      metadata: { platform: this.adapter.name || 'unknown' },
    };

    sendToWorker(MSG.CAPTURE_CONVERSATION, { data }).catch((err) => {
      console.warn('[npu-webhook] Capture send failed, will retry next time:', err);
      this._seen.delete(hash); // 允许下次重新捕获
    });
  }

  /** 长对话请求 Ollama 摘要后入库 */
  _summarizeAndSave(title, fullContent) {
    const data = {
      title,
      content: fullContent,
      source_type: 'ai_chat',
      url: location.href,
      domain: location.hostname,
      metadata: { platform: this.adapter.name || 'unknown', has_summary: 'pending' },
    };

    sendToWorker(MSG.SUMMARIZE_AND_SAVE, { data }).catch((err) => {
      console.warn('[npu-webhook] Summarize failed, saving full text:', err);
      // 摘要失败回退到直接保存，但 has_summary 需改为 false
      this._directSave(title, fullContent);
    });
  }
}
