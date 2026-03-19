import { h } from 'preact';
import { useState } from 'preact/hooks';
import { MSG, sendToWorker } from '../../shared/messages.js';

const SOURCE_TYPES = [
  { value: '', label: '全部' },
  { value: 'ai_chat', label: 'AI 对话' },
  { value: 'note', label: '笔记' },
  { value: 'webpage', label: '网页' },
  { value: 'file', label: '文件' },
];

const TYPE_LABELS = {
  ai_chat: '💬 AI对话',
  note: '📝 笔记',
  webpage: '🌐 网页',
  file: '📄 文件',
  selection: '✂️ 摘录',
};

export default function SearchPage() {
  const [query, setQuery] = useState('');
  const [sourceType, setSourceType] = useState('');
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [searched, setSearched] = useState(false);
  const [expanded, setExpanded] = useState(new Set()); // 支持多条同时展开

  const toggleExpand = (id) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  const doSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setError('');
    setSearched(false);
    try {
      const res = await sendToWorker(MSG.SEARCH, {
        query: query.trim(),
        top_k: 20,
        source_types: sourceType || undefined,
      });
      setResults(res?.results || []);
      setSearched(true);
    } catch (err) {
      console.error('Search failed:', err);
      setError('搜索失败，请检查后端连接');
      setResults([]);
    }
    setLoading(false);
  };

  const onKeyDown = (e) => {
    if (e.key === 'Enter') doSearch();
  };

  return (
    <div>
      <div class="sp-search">
        <input
          value={query}
          onInput={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="搜索知识库..."
          disabled={loading}
        />
        <select value={sourceType} onChange={(e) => setSourceType(e.target.value)} disabled={loading}>
          {SOURCE_TYPES.map((t) => (
            <option key={t.value} value={t.value}>{t.label}</option>
          ))}
        </select>
        <button onClick={doSearch} disabled={loading || !query.trim()}>
          {loading ? '搜索中...' : '搜索'}
        </button>
      </div>

      {error && <div class="sp-error">{error}</div>}

      {!loading && searched && results.length === 0 && !error && (
        <div class="sp-empty">未找到相关知识</div>
      )}

      {results.map((r) => {
        const isOpen = expanded.has(r.id);
        const preview = (r.content || '').slice(0, 120);
        const hasMore = (r.content || '').length > 120;
        return (
          <div key={r.id} class="sp-card" onClick={() => toggleExpand(r.id)}>
            <div class="sp-card__title">{r.title || '无标题'}</div>
            <div class="sp-card__meta">
              <span>{TYPE_LABELS[r.source_type] || r.source_type}</span>
              {r.score != null && <span>{(r.score * 100).toFixed(0)}%</span>}
              {r.created_at && <span>{new Date(r.created_at).toLocaleDateString('zh-CN')}</span>}
            </div>
            <div class="sp-card__content">
              {isOpen ? r.content : (hasMore ? preview + '…' : preview)}
            </div>
            {hasMore && (
              <div class="sp-card__toggle">{isOpen ? '收起 ▲' : '展开 ▼'}</div>
            )}
          </div>
        );
      })}
    </div>
  );
}
