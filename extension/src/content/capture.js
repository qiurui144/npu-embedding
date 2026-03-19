/**
 * 对话捕获 — MutationObserver 监听 AI 回答，配对后推送摘要到 Worker
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
    this._nodeTimers = new Map(); // per-node debounce，防止多消息并发丢失
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
    for (const timer of this._nodeTimers.values()) clearTimeout(timer);
    this._nodeTimers.clear();
  }

  _markExisting() {
    const nodes = document.querySelectorAll(this.adapter.messages);
    for (const node of nodes) {
      const msg = this.adapter.extractMessage(node);
      if (msg) this._seen.add(hashStr(msg.content));
    }
  }

  _handleNode(node) {
    const msgNode = node.matches?.(this.adapter.messages) ? node : node.querySelector?.(this.adapter.messages);
    if (!msgNode) return;

    // per-node debounce: 每个消息节点独立计时，互不影响
    const nodeKey = msgNode;
    if (this._nodeTimers.has(nodeKey)) clearTimeout(this._nodeTimers.get(nodeKey));
    this._nodeTimers.set(nodeKey, setTimeout(() => {
      this._nodeTimers.delete(nodeKey);
      this._processNode(msgNode);
    }, 2000));
  }

  _processNode(node) {
    if (!this.adapter.isComplete(node)) {
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
    const fullContent = `用户: ${user.content}\n\n助手: ${assistant.content}`;

    // 短对话直接入库，长对话请求摘要后入库
    if (fullContent.length < 500) {
      this._directSave(title, fullContent);
    } else {
      this._summarizeAndSave(title, fullContent);
    }
  }

  /** 短对话直接入库 */
  _directSave(title, content) {
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

  /** 长对话请求 Ollama 摘要后入库 */
  _summarizeAndSave(title, fullContent) {
    const data = {
      title,
      content: fullContent,
      source_type: 'ai_chat',
      url: location.href,
      domain: location.hostname,
      metadata: { platform: this.adapter.name || 'unknown', has_summary: 'true' },
    };

    sendToWorker(MSG.SUMMARIZE_AND_SAVE, { data }).catch((err) => {
      // 摘要失败时回退到直接保存全文
      console.warn('[npu-webhook] Summarize failed, saving full text:', err);
      this._directSave(title, fullContent);
    });
  }
}
