/**
 * Content Script 入口 — 整合 capture + injector + indicator
 */

import { detectPlatform } from './detector.js';
import { ConversationCapture } from './capture.js';
import { PrefixInjector } from './injector.js';
import { StatusIndicator } from './indicator.js';
import { MSG, sendToWorker } from '../shared/messages.js';

const platform = detectPlatform();
if (platform) {
  console.log(`[npu-webhook] Detected platform: ${platform.name}`);

  const indicator = new StatusIndicator();
  const capture = new ConversationCapture(platform);
  const injector = new PrefixInjector(platform);

  async function init() {
    indicator.mount();
    indicator.setState('processing');

    // 检查后端连接
    try {
      const status = await sendToWorker(MSG.GET_STATUS);
      if (status?.online) {
        indicator.setState('captured');
      } else {
        indicator.setState('offline');
      }

      // 获取注入设置
      if (status?.injection_enabled === false) {
        injector.toggle(false);
      }
    } catch {
      indicator.setState('offline');
    }

    // 启动捕获和注入
    capture.start();
    injector.attach();
  }

  // 监听 worker 消息
  chrome.runtime.onMessage.addListener((msg) => {
    if (msg.type === MSG.TOGGLE_INJECTION) {
      injector.toggle(msg.enabled);
    } else if (msg.type === MSG.SETTINGS_UPDATED) {
      // 重新加载设置
      sendToWorker(MSG.GET_STATUS).then((status) => {
        if (status?.online) {
          indicator.setState('captured');
        } else {
          indicator.setState('offline');
        }
      }).catch(() => indicator.setState('offline'));
    }
  });

  init();
}
