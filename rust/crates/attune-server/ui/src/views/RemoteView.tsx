/** Remote 视图 · Phase 4 占位 */

import type { JSX } from 'preact';
import { EmptyState } from '../components';

export function RemoteView(): JSX.Element {
  return (
    <div style={{ padding: 'var(--space-5)', height: '100%' }}>
      <h2 style={{ fontSize: 'var(--text-xl)', fontWeight: 600, margin: 0, marginBottom: 'var(--space-4)' }}>
        🔗 远程目录
      </h2>
      <EmptyState
        icon="🔗"
        title="还没绑定任何远程目录"
        description="连接 Nextcloud / 自建 WebDAV，自动同步文件进知识库"
        actions={[{ label: '添加目录', onClick: () => {}, variant: 'primary' }]}
      />
    </div>
  );
}
