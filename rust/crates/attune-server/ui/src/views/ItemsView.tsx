/** Items 视图 · Phase 4 占位 */

import type { JSX } from 'preact';
import { EmptyState } from '../components';
import { t } from '../i18n';

export function ItemsView(): JSX.Element {
  return (
    <div style={{ padding: 'var(--space-5)', height: '100%' }}>
      <h2 style={{ fontSize: 'var(--text-xl)', fontWeight: 600, margin: 0, marginBottom: 'var(--space-4)' }}>
        📄 条目
      </h2>
      <EmptyState
        icon="📂"
        title={t('empty.items.title')}
        description={t('empty.items.desc')}
        actions={[
          { label: '上传文件', onClick: () => {}, variant: 'primary' },
          { label: '绑定文件夹', onClick: () => {}, variant: 'secondary' },
        ]}
      />
    </div>
  );
}
