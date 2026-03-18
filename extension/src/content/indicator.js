/**
 * 页面状态指示器 — 右下角小圆点
 * disabled(灰) / processing(黄) / captured(绿) / offline(红)
 */

import { MSG, sendToWorker } from '../shared/messages.js';

const STATES = ['disabled', 'processing', 'captured', 'offline'];

export class StatusIndicator {
  constructor() {
    this._el = null;
    this._tooltip = null;
    this._state = 'disabled';
  }

  mount() {
    if (this._el) return;

    this._el = document.createElement('div');
    this._el.className = 'npu-webhook-indicator npu-webhook-indicator--disabled';
    this._el.title = 'npu-webhook: 未连接';

    this._el.addEventListener('click', () => {
      sendToWorker(MSG.OPEN_SIDEPANEL).catch(() => {});
    });

    document.body.appendChild(this._el);
  }

  setState(state) {
    if (!STATES.includes(state)) return;
    if (!this._el) return;
    this._state = state;

    // 移除旧状态 class，添加新状态
    for (const s of STATES) {
      this._el.classList.remove(`npu-webhook-indicator--${s}`);
    }
    this._el.classList.add(`npu-webhook-indicator--${state}`);

    const labels = {
      disabled: '未启用',
      processing: '处理中...',
      captured: '已连接',
      offline: '离线',
    };
    this._el.title = `npu-webhook: ${labels[state] || state}`;
  }

  unmount() {
    if (this._el) {
      this._el.remove();
      this._el = null;
    }
  }
}
