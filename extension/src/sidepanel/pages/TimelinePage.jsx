import { h } from 'preact';
import { useState, useEffect, useMemo } from 'preact/hooks';
import { MSG, sendToWorker } from '../../shared/messages.js';

const LIMIT = 20;

export default function TimelinePage() {
  const [items, setItems] = useState([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState('');
  const [deletingIds, setDeletingIds] = useState(new Set());
  const [backendUrl, setBackendUrl] = useState('http://localhost:18900');

  const load = async (newOffset = 0) => {
    if (newOffset === 0) {
      setLoading(true);
    } else {
      setLoadingMore(true);
    }
    setError('');
    try {
      const res = await sendToWorker(MSG.GET_ITEMS, { offset: newOffset, limit: LIMIT });
      if (newOffset === 0) {
        setItems(res?.items || []);
      } else {
        setItems((prev) => [...prev, ...(res?.items || [])]);
      }
      setTotal(res?.total || 0);
      setOffset(newOffset);
    } catch (err) {
      console.error('Load items failed:', err);
      setError('加载失败，请检查后端连接');
    }
    setLoading(false);
    setLoadingMore(false);
  };

  useEffect(() => {
    sendToWorker(MSG.GET_SETTINGS).then((s) => {
      if (s?.backendUrl) setBackendUrl(s.backendUrl.replace(/\/+$/, ''));
    }).catch(() => {});
    load();
  }, []);

  const deleteItem = async (id, e) => {
    e.stopPropagation();
    if (deletingIds.has(id)) return;

    setDeletingIds((prev) => new Set([...prev, id]));
    try {
      const baseUrl = backendUrl;
      const resp = await fetch(`${baseUrl}/api/v1/items/${id}`, { method: 'DELETE' });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      // 删除成功后再更新 UI（非乐观更新，避免误删）
      setItems((prev) => prev.filter((it) => it.id !== id));
      setTotal((t) => Math.max(0, t - 1));
    } catch (err) {
      console.error('Delete failed:', err);
      // 删除失败：UI 不变，显示错误
      setError(`删除失败: ${err.message}`);
    } finally {
      setDeletingIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  // useMemo 缓存分组结果，避免每次 render 重新计算
  const groups = useMemo(() => {
    const g = {};
    for (const item of items) {
      const date = item.created_at
        ? new Date(item.created_at).toLocaleDateString('zh-CN')
        : '未知日期';
      if (!g[date]) g[date] = [];
      g[date].push(item);
    }
    return g;
  }, [items]);

  const hasMore = items.length < total;

  return (
    <div>
      {loading && <div class="sp-loading">加载中...</div>}

      {!loading && error && <div class="sp-error">{error}</div>}

      {!loading && items.length === 0 && !error && (
        <div class="sp-empty">暂无知识条目</div>
      )}

      {Object.entries(groups).map(([date, dateItems]) => (
        <div key={date}>
          <div class="sp-date-group">{date}</div>
          {dateItems.map((item) => {
            const isDeleting = deletingIds.has(item.id);
            return (
              <div key={item.id} class={`sp-card${isDeleting ? ' sp-card--deleting' : ''}`}>
                <div class="sp-card__title">{item.title || '无标题'}</div>
                <div class="sp-card__meta">
                  <span>{item.source_type}</span>
                  {item.domain && <span>{item.domain}</span>}
                </div>
                <div class="sp-card__content">
                  {(item.content || '').slice(0, 150)}{(item.content || '').length > 150 ? '…' : ''}
                </div>
                <div class="sp-card__actions">
                  <button
                    onClick={(e) => deleteItem(item.id, e)}
                    disabled={isDeleting}
                  >
                    {isDeleting ? '删除中...' : '删除'}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ))}

      {hasMore && (
        <button
          class="sp-refresh"
          onClick={() => load(offset + LIMIT)}
          disabled={loadingMore}
        >
          {loadingMore ? '加载中...' : `加载更多 (${items.length}/${total})`}
        </button>
      )}
    </div>
  );
}
