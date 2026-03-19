/**
 * 知识注入器 — 上下文感知 + 阈值过滤 + Ctrl+K 手动触发 + 打字预取
 */

import { MSG, sendToWorker } from '../shared/messages.js';

const MIN_SCORE = 0.3;   // 低于此分数的结果不注入
const MAX_CONTEXT_TURNS = 3;  // 最近 N 轮对话作为上下文
const PREFETCH_DEBOUNCE = 400; // 打字停止 400ms 后触发预取（ms）
const MIN_QUERY_LEN = 15;     // 短于此长度跳过注入（闲聊/简单指令）

// 不需要知识注入的高频无意义词（正则匹配整句）
const SKIP_PATTERNS = [
  /^(你好|hi|hello|ok|好的|谢谢|感谢|继续|再说一遍|重新生成|stop|clear)$/i,
  /^[\p{Emoji}\s]+$/u,
];

/**
 * 简单意图分类 → 返回优先 source_types 列表
 * 让后端按意图偏向搜索更相关的知识类型
 */
function _detectIntent(text) {
  const t = text.toLowerCase();
  // 编程/代码类
  if (/```|def |function |import |class |error:|traceback|syntax/i.test(text)
      || /\.(py|js|ts|go|java|rs|cpp)\b|npm|pip|git\s/i.test(text)) {
    return ['file', 'note', 'ai_chat'];
  }
  // 翻译类
  if (/翻译|translate|英文|中文|日文|french|spanish/i.test(t)) {
    return null; // 不限制类型，但注入知识可能帮助术语翻译
  }
  // 知识问答/解释类（默认，优先历史对话和笔记）
  if (/是什么|怎么|为什么|how|what|why|explain|介绍|区别|对比/i.test(t)) {
    return ['ai_chat', 'note', 'webpage'];
  }
  return null; // 不做限制
}

export class PrefixInjector {
  constructor(adapter) {
    this.adapter = adapter;
    this._enabled = true;
    this._injecting = false;
    this._clickHandler = null;
    this._keyHandler = null;
    this._inputHandler = null;
    this._prefetchTimer = null;
  }

  attach() {
    this._clickHandler = (e) => this._onSendClick(e);
    this._keyHandler = (e) => this._onKeyDown(e);
    this._inputHandler = (e) => this._onInput(e);

    // capture phase 拦截发送按钮
    document.addEventListener('click', this._clickHandler, true);
    // Ctrl+K 手动触发
    document.addEventListener('keydown', this._keyHandler, true);
    // 打字时预取（input 事件冒泡，监听 document 即可）
    document.addEventListener('input', this._inputHandler, true);
    console.log('[npu-webhook] Injector attached (auto + Ctrl+K + prefetch)');
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
    if (this._inputHandler) {
      document.removeEventListener('input', this._inputHandler, true);
      this._inputHandler = null;
    }
    clearTimeout(this._prefetchTimer);
  }

  toggle(enabled) {
    this._enabled = enabled;
  }

  /** 是否应跳过注入（闲聊/简单指令/纯 Emoji） */
  _shouldSkip(text) {
    if (text.length < MIN_QUERY_LEN) return true;
    return SKIP_PATTERNS.some((re) => re.test(text.trim()));
  }

  /** 打字时 debounce 预取，减少注入延迟 */
  _onInput() {
    if (!this._enabled || this._injecting) return;

    clearTimeout(this._prefetchTimer);
    this._prefetchTimer = setTimeout(() => {
      const inputBox = document.querySelector(this.adapter.inputBox);
      if (!inputBox) return;
      const text = inputBox.innerText?.trim() || inputBox.textContent?.trim() || '';
      if (this._shouldSkip(text)) return;

      const context = this._collectContext();
      const sourceTypes = _detectIntent(text);
      sendToWorker(MSG.PREFETCH, {
        query: text,
        context,
        min_score: MIN_SCORE,
        top_k: 3,
        source_types: sourceTypes,
      }).catch(() => {});
    }, PREFETCH_DEBOUNCE);
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
    const sourceTypes = _detectIntent(text);
    try {
      const res = await sendToWorker(MSG.SEARCH_RELEVANT, {
        query: text,
        context,
        min_score: MIN_SCORE,
        top_k: 5,
        source_types: sourceTypes,
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

    // 跳过短文本、闲聊、纯 Emoji
    if (this._shouldSkip(originalText)) return;

    e.stopImmediatePropagation();
    e.preventDefault();
    this._injecting = true;

    try {
      const context = this._collectContext();
      const sourceTypes = _detectIntent(originalText);

      const res = await sendToWorker(MSG.SEARCH_RELEVANT, {
        query: originalText,
        context,
        min_score: MIN_SCORE,
        top_k: 3,
        source_types: sourceTypes,
      });

      // 只在有高质量结果时注入
      if (res?.results?.length > 0) {
        const prefix = this._buildPrefix(res.results, originalText);
        this.adapter.setInputContent(inputBox, prefix);
      }
    } catch (err) {
      console.warn('[npu-webhook] Inject search failed:', err);
    }

    // 保持 _injecting=true 防止递归，click 后延迟 reset
    sendBtn.click();
    setTimeout(() => { this._injecting = false; }, 100);
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
