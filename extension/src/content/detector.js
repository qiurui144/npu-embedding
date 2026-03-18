/**
 * AI 平台检测 + DOM 适配器（ChatGPT / Claude / Gemini）
 */

export const ADAPTERS = {
  chatgpt: {
    match: () => location.hostname === 'chatgpt.com',
    inputBox: '#prompt-textarea',
    sendButton: 'button[data-testid="send-button"]',
    messages: '[data-message-author-role]',
    messagesContainer: '[role="presentation"]',

    extractMessage(node) {
      const role = node.getAttribute('data-message-author-role');
      if (!role) return null;
      const md = node.querySelector('.markdown');
      const content = md ? md.innerText.trim() : node.innerText.trim();
      if (!content) return null;
      return { role: role === 'user' ? 'user' : 'assistant', content };
    },

    isComplete(node) {
      return !node.querySelector('.result-streaming');
    },

    setInputContent(element, text) {
      element.innerText = text;
      element.dispatchEvent(new Event('input', { bubbles: true }));
    },
  },

  claude: {
    match: () => location.hostname === 'claude.ai',
    inputBox: '[contenteditable="true"].ProseMirror',
    sendButton: 'button[aria-label="Send Message"]',
    messages: '[data-testid="conversation-turn"]',
    messagesContainer: '[data-testid="conversation-turn-list"]',

    extractMessage(node) {
      const isUser = node.querySelector('[data-testid="user-message"]');
      const isAssistant = node.querySelector('[data-testid="assistant-message"]');
      if (!isUser && !isAssistant) return null;
      const role = isUser ? 'user' : 'assistant';
      const contentEl = isUser || isAssistant;
      const content = contentEl.textContent.trim();
      if (!content) return null;
      return { role, content };
    },

    isComplete(node) {
      return !node.querySelector('[data-is-streaming="true"]');
    },

    setInputContent(element, text) {
      element.innerHTML = `<p>${text}</p>`;
      element.dispatchEvent(new Event('input', { bubbles: true }));
    },
  },

  gemini: {
    match: () => location.hostname === 'gemini.google.com',
    inputBox: '.ql-editor',
    sendButton: 'button[aria-label="Send message"]',
    messages: '.conversation-container',
    messagesContainer: 'chat-window, main',

    extractMessage(node) {
      const isUser = node.querySelector('.query-content, .user-query');
      const isModel = node.querySelector('.model-response-text, .response-content');
      if (!isUser && !isModel) return null;
      const role = isUser ? 'user' : 'assistant';
      const contentEl = isUser || isModel;
      const content = contentEl.textContent.trim();
      if (!content) return null;
      return { role, content };
    },

    isComplete(node) {
      return !node.querySelector('.loading-indicator, .response-streaming');
    },

    setInputContent(element, text) {
      element.innerHTML = `<p>${text}</p>`;
      element.dispatchEvent(new Event('input', { bubbles: true }));
    },
  },
};

export function detectPlatform() {
  for (const [name, adapter] of Object.entries(ADAPTERS)) {
    if (adapter.match()) return { name, ...adapter };
  }
  return null;
}
