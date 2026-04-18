/** Knowledge 视图 · Phase 4 占位 */

import type { JSX } from 'preact';
import { EmptyState } from '../components';

export function KnowledgeView(): JSX.Element {
  return (
    <div style={{ padding: 'var(--space-5)', height: '100%' }}>
      <h2 style={{ fontSize: 'var(--text-xl)', fontWeight: 600, margin: 0, marginBottom: 'var(--space-4)' }}>
        📊 知识全景
      </h2>
      <EmptyState
        icon="📊"
        title="还没发现聚类"
        description="需要至少 20 条记录，后台 HDBSCAN 聚类会自动发现主题群组"
      />
    </div>
  );
}
