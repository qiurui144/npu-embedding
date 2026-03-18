/**
 * 对话捕获 — MutationObserver 监听 AI 回答，配对后推送到 Worker
 */

import { MSG, sendToWorker } from '../shared/messages.js';

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
    this._pendingUser = null;
    this._debounceTimer = null;
  }

  start() {
    const container = document.querySelector(this.adapter.messagesContainer);
    if (!container) {
      // 容器还没出现，延迟重试
      setTimeout(() => this.start(), 2000);
      return;
    }

    // 扫描已有消息不捕获，只标记已见
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
    if (this._debounceTimer) clearTimeout(this._debounceTimer);
  }

  _markExisting() {
    const nodes = document.querySelectorAll(this.adapter.messages);
    for (const node of nodes) {
      const msg = this.adapter.extractMessage(node);
      if (msg) this._seen.add(hashStr(msg.content));
    }
  }

  _handleNode(node) {
    // 查找消息节点（可能是子节点触发）
    const msgNode = node.matches?.(this.adapter.messages) ? node : node.querySelector?.(this.adapter.messages);
    if (!msgNode) return;

    // 流式输出 debounce
    if (this._debounceTimer) clearTimeout(this._debounceTimer);
    this._debounceTimer = setTimeout(() => this._processNode(msgNode), 2000);
  }

  _processNode(node) {
    if (!this.adapter.isComplete(node)) {
      // 还没完成，继续等
      this._debounceTimer = setTimeout(() => this._processNode(node), 1000);
      return;
    }

    const msg = this.adapter.extractMessage(node);
    if (!msg) return;

    const h = hashStr(msg.content);
    if (this._seen.has(h)) return;
    this._seen.add(h);

    if (msg.role === 'user') {
      this._pendingUser = msg;
    } else if (msg.role === 'assistant' && this._pendingUser) {
      this._sendPair(this._pendingUser, msg);
      this._pendingUser = null;
    }
  }

  _sendPair(user, assistant) {
    const title = user.content.slice(0, 100);
    const content = `用户: ${user.content}\n\n助手: ${assistant.content}`;
    const data = {
      title,
      content,
      source_type: 'ai_chat',
      url: location.href,
      domain: location.hostname,
      metadata: { platform: this.adapter.name || 'unknown' },
    };

    sendToWorker(MSG.CAPTURE_CONVERSATION, { data }).catch((err) => {
      console.warn('[npu-webhook] Capture send failed:', err);
    });
  }
}
