/** Chat 视图 · Phase 4 占位（Phase 5 会实现实际对话） */

import type { JSX } from 'preact';
import { EmptyState } from '../components';
import { t } from '../i18n';
import { toast } from '../components/Toast';

export function ChatView(): JSX.Element {
  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <header
        style={{
          padding: 'var(--space-4) var(--space-5)',
          borderBottom: '1px solid var(--color-border)',
          fontSize: 'var(--text-sm)',
          color: 'var(--color-text-secondary)',
        }}
      >
        新对话 · 模型: <code style={{ color: 'var(--color-text)' }}>qwen2.5:3b</code>
      </header>

      <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <EmptyState
          icon="💬"
          title={t('empty.chat.title')}
          description={t('empty.chat.desc')}
          examples={[
            '帮我检索最近的合同条款',
            '这份文件讲了什么？',
            '有类似的先行技术吗',
          ]}
          onExampleClick={(ex) => toast('info', `示例点击（Phase 5 接入）：${ex}`)}
        />
      </div>

      <footer
        style={{
          padding: 'var(--space-3) var(--space-5)',
          borderTop: '1px solid var(--color-border)',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-2)',
            padding: 'var(--space-3)',
            background: 'var(--color-bg)',
            border: '1px solid var(--color-border)',
            borderRadius: 'var(--radius-lg)',
            color: 'var(--color-text-secondary)',
            fontSize: 'var(--text-sm)',
          }}
        >
          <span style={{ flex: 1 }}>{t('chat.input.placeholder')}</span>
          <span style={{ fontSize: 'var(--text-xs)' }}>~0 tok · 本地</span>
        </div>
      </footer>
    </div>
  );
}
