import { h } from 'preact';
import { useState, useEffect } from 'preact/hooks';
import { MSG, sendToWorker } from '../../shared/messages.js';

export default function TimelinePage() {
  const [items, setItems] = useState([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);
  const LIMIT = 20;

  const load = async (newOffset = 0) => {
    setLoading(true);
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
    }
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const deleteItem = async (id, e) => {
    e.stopPropagation();
    try {
      const settings = await sendToWorker(MSG.GET_SETTINGS);
      const baseUrl = (settings?.backendUrl || 'http://localhost:18900').replace(/\/+$/, '');
      const resp = await fetch(`${baseUrl}/api/v1/items/${id}`, { method: 'DELETE' });
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      setItems((prev) => prev.filter((it) => it.id !== id));
      setTotal((t) => t - 1);
    } catch (err) {
      console.error('Delete failed:', err);
    }
  };

  // 按日期分组
  const groups = {};
  for (const item of items) {
    const date = item.created_at ? new Date(item.created_at).toLocaleDateString('zh-CN') : '未知日期';
    if (!groups[date]) groups[date] = [];
    groups[date].push(item);
  }

  const hasMore = offset + LIMIT < total;

  return (
    <div>
      {loading && items.length === 0 && <div class="sp-loading">加载中...</div>}

      {!loading && items.length === 0 && <div class="sp-empty">暂无知识条目</div>}

      {Object.entries(groups).map(([date, dateItems]) => (
        <div key={date}>
          <div class="sp-date-group">{date}</div>
          {dateItems.map((item) => (
            <div key={item.id} class="sp-card">
              <div class="sp-card__title">{item.title || '无标题'}</div>
              <div class="sp-card__meta">
                <span>{item.source_type}</span>
                {item.domain && <span>{item.domain}</span>}
              </div>
              <div class="sp-card__content">
                {(item.content || '').slice(0, 150)}
              </div>
              <div class="sp-card__actions">
                <button onClick={(e) => deleteItem(item.id, e)}>删除</button>
              </div>
            </div>
          ))}
        </div>
      ))}

      {hasMore && (
        <button class="sp-refresh" onClick={() => load(offset + LIMIT)} disabled={loading}>
          {loading ? '加载中...' : '加载更多'}
        </button>
      )}
    </div>
  );
}
