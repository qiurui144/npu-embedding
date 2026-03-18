/**
 * 无感前缀注入 — 拦截发送按钮，查询知识库并拼接前缀
 */

import { MSG, sendToWorker } from '../shared/messages.js';

export class PrefixInjector {
  constructor(adapter) {
    this.adapter = adapter;
    this._enabled = true;
    this._injecting = false;
    this._clickHandler = null;
  }

  attach() {
    this._clickHandler = (e) => this._onSendClick(e);

    // capture phase 拦截，在平台自己的 handler 之前
    document.addEventListener('click', this._clickHandler, true);
    console.log('[npu-webhook] Injector attached');
  }

  detach() {
    if (this._clickHandler) {
      document.removeEventListener('click', this._clickHandler, true);
      this._clickHandler = null;
    }
  }

  toggle(enabled) {
    this._enabled = enabled;
  }

  async _onSendClick(e) {
    if (!this._enabled || this._injecting) return;

    // 检查点击的是否是发送按钮（或其子元素）
    const sendBtn = e.target.closest(this.adapter.sendButton);
    if (!sendBtn) return;

    const inputBox = document.querySelector(this.adapter.inputBox);
    if (!inputBox) return;

    const originalText = inputBox.innerText?.trim() || inputBox.textContent?.trim() || '';
    if (!originalText) return;

    // 阻止原始发送
    e.stopImmediatePropagation();
    e.preventDefault();

    this._injecting = true;

    try {
      const res = await sendToWorker(MSG.SEARCH_RELEVANT, {
        query: originalText,
        top_k: 3,
      });

      if (res?.results?.length > 0) {
        const prefix = this._buildPrefix(res.results, originalText);
        this.adapter.setInputContent(inputBox, prefix);
      }
    } catch (err) {
      console.warn('[npu-webhook] Inject search failed:', err);
    }

    // 释放点击，让发送继续
    this._injecting = false;
    sendBtn.click();
  }

  _buildPrefix(results, originalQuestion) {
    const sections = [];

    const typeLabels = {
      ai_chat: '历史对话',
      note: '个人笔记',
      webpage: '网页摘录',
      file: '本地文件',
    };

    for (const r of results) {
      const icon = r.source_type === 'ai_chat' ? '💬' : r.source_type === 'note' ? '📝' : '📄';
      const label = typeLabels[r.source_type] || '知识条目';
      const snippet = r.content.slice(0, 300);
      sections.push(`${icon} ${label}: ${snippet}`);
    }

    return [
      '[以下是来自个人知识库的相关参考，请结合回答]',
      ...sections,
      '---',
      originalQuestion,
    ].join('\n');
  }
}
