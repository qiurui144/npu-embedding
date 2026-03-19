/**
 * 知识注入器 — 上下文感知 + 阈值过滤 + Ctrl+K 手动触发
 */

import { MSG, sendToWorker } from '../shared/messages.js';

const MIN_SCORE = 0.3;  // 低于此分数的结果不注入
const MAX_CONTEXT_TURNS = 3;  // 最近 N 轮对话作为上下文

export class PrefixInjector {
  constructor(adapter) {
    this.adapter = adapter;
    this._enabled = true;
    this._injecting = false;
    this._clickHandler = null;
    this._keyHandler = null;
  }

  attach() {
    this._clickHandler = (e) => this._onSendClick(e);
    this._keyHandler = (e) => this._onKeyDown(e);

    // capture phase 拦截发送按钮
    document.addEventListener('click', this._clickHandler, true);
    // Ctrl+K 手动触发
    document.addEventListener('keydown', this._keyHandler, true);
    console.log('[npu-webhook] Injector attached (auto + Ctrl+K)');
  }

  detach() {
    if (this._clickHandler) {
      document.removeEventListener('click', this._clickHandler, true);
      this._clickHandler = null;
    }
    if (this._keyHandler) {
      document.removeEventListener('keydown', this._keyHandler, true);
      this._keyHandler = null;
    }
  }

  toggle(enabled) {
    this._enabled = enabled;
  }

  /** 收集最近 N 轮对话作为搜索上下文 */
  _collectContext() {
    const context = [];
    try {
      const nodes = document.querySelectorAll(this.adapter.messages);
      const recent = Array.from(nodes).slice(-MAX_CONTEXT_TURNS * 2);
      for (const node of recent) {
        const msg = this.adapter.extractMessage(node);
        if (msg?.content) {
          context.push(msg.content.slice(0, 200));
        }
      }
    } catch { /* 静默失败 */ }
    return context;
  }

  /** Ctrl+K 手动触发注入 */
  _onKeyDown(e) {
    if (e.ctrlKey && e.key === 'k') {
      e.preventDefault();
      e.stopImmediatePropagation();
      this._manualInject();
    }
  }

  async _manualInject() {
    const inputBox = document.querySelector(this.adapter.inputBox);
    if (!inputBox) return;

    const text = inputBox.innerText?.trim() || inputBox.textContent?.trim() || '';
    if (!text) return;

    const context = this._collectContext();
    try {
      const res = await sendToWorker(MSG.SEARCH_RELEVANT, {
        query: text,
        context,
        min_score: MIN_SCORE,
        top_k: 5,
      });

      if (res?.results?.length > 0) {
        const prefix = this._buildPrefix(res.results, text);
        this.adapter.setInputContent(inputBox, prefix);
      }
    } catch (err) {
      console.warn('[npu-webhook] Manual inject failed:', err);
    }
  }

  /** 自动注入：拦截发送按钮 */
  async _onSendClick(e) {
    if (!this._enabled || this._injecting) return;

    const sendBtn = e.target.closest(this.adapter.sendButton);
    if (!sendBtn) return;

    const inputBox = document.querySelector(this.adapter.inputBox);
    if (!inputBox) return;

    const originalText = inputBox.innerText?.trim() || inputBox.textContent?.trim() || '';
    if (!originalText) return;

    // 短文本（< 10 字）跳过注入（闲聊/简单指令）
    if (originalText.length < 10) return;

    e.stopImmediatePropagation();
    e.preventDefault();
    this._injecting = true;

    try {
      const context = this._collectContext();

      const res = await sendToWorker(MSG.SEARCH_RELEVANT, {
        query: originalText,
        context,
        min_score: MIN_SCORE,
        top_k: 3,
      });

      // 只在有高质量结果时注入
      if (res?.results?.length > 0) {
        const prefix = this._buildPrefix(res.results, originalText);
        this.adapter.setInputContent(inputBox, prefix);
      }
    } catch (err) {
      console.warn('[npu-webhook] Inject search failed:', err);
    }

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
      selection: '选中摘录',
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
