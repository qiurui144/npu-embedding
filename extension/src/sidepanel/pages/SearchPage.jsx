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

export default function SearchPage() {
  const [query, setQuery] = useState('');
  const [sourceType, setSourceType] = useState('');
  const [results, setResults] = useState([]);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState(null);

  const doSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    try {
      const res = await sendToWorker(MSG.SEARCH, {
        query: query.trim(),
        top_k: 20,
        source_types: sourceType || undefined,
      });
      setResults(res?.results || []);
    } catch (err) {
      console.error('Search failed:', err);
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
        />
        <select value={sourceType} onChange={(e) => setSourceType(e.target.value)}>
          {SOURCE_TYPES.map((t) => (
            <option key={t.value} value={t.value}>{t.label}</option>
          ))}
        </select>
        <button onClick={doSearch}>搜索</button>
      </div>

      {loading && <div class="sp-loading">搜索中...</div>}

      {!loading && results.length === 0 && query && (
        <div class="sp-empty">无结果</div>
      )}

      {results.map((r) => (
        <div key={r.id} class="sp-card" onClick={() => setExpanded(expanded === r.id ? null : r.id)}>
          <div class="sp-card__title">{r.title || '无标题'}</div>
          <div class="sp-card__meta">
            <span>{r.source_type}</span>
            {r.score != null && <span>相关度: {(r.score * 100).toFixed(0)}%</span>}
            {r.created_at && <span>{new Date(r.created_at).toLocaleDateString()}</span>}
          </div>
          <div class="sp-card__content">
            {expanded === r.id ? r.content : (r.content || '').slice(0, 120) + '...'}
          </div>
        </div>
      ))}
    </div>
  );
}
