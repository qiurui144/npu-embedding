import { h } from 'preact';
import { useState, useEffect } from 'preact/hooks';
import { MSG, sendToWorker } from '../../shared/messages.js';

export default function StatusPage() {
  const [status, setStatus] = useState(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
    setLoading(true);
    try {
      const res = await sendToWorker(MSG.GET_STATUS);
      setStatus(res);
    } catch (err) {
      console.error('Status fetch failed:', err);
      setStatus({ online: false });
    }
    setLoading(false);
  };

  useEffect(() => { refresh(); }, []);

  if (loading) return <div class="sp-loading">加载中...</div>;

  const online = status?.online;

  return (
    <div>
      <div class="sp-status-grid">
        <div class="sp-status-item">
          <div class="sp-status-item__label">连接状态</div>
          <div class="sp-status-item__value" style={{ color: online ? '#22c55e' : '#ef4444' }}>
            {online ? '在线' : '离线'}
          </div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">版本</div>
          <div class="sp-status-item__value">{status?.version || '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">设备</div>
          <div class="sp-status-item__value">{status?.device || '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">模型</div>
          <div class="sp-status-item__value" style={{ fontSize: '12px' }}>{status?.model_name || '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">知识条目</div>
          <div class="sp-status-item__value">{status?.total_items ?? '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">向量数</div>
          <div class="sp-status-item__value">{status?.total_vectors ?? '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">待处理 Embedding</div>
          <div class="sp-status-item__value">{status?.pending_embeddings ?? '-'}</div>
        </div>
        <div class="sp-status-item">
          <div class="sp-status-item__label">监控目录</div>
          <div class="sp-status-item__value">{status?.bound_directories ?? '-'}</div>
        </div>
      </div>

      <button class="sp-refresh" onClick={refresh}>刷新</button>
    </div>
  );
}
