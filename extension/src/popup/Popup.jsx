import { h, render } from 'preact';
import { useState, useEffect } from 'preact/hooks';
import { MSG, sendToWorker } from '../shared/messages.js';
// G5 隐私面板（W3 batch B, 2026-04-27）— per R02 P0-1 修复 Privacy.jsx 死代码
import { Privacy } from './Privacy.jsx';

const styles = {
  container: { padding: '16px', fontFamily: 'system-ui, sans-serif', fontSize: '14px' },
  header: { display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '12px' },
  title: { margin: 0, fontSize: '16px', fontWeight: 600 },
  dot: (online) => ({
    width: '8px', height: '8px', borderRadius: '50%',
    backgroundColor: online ? '#22c55e' : '#ef4444',
    flexShrink: 0,
  }),
  stats: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', marginBottom: '12px' },
  stat: {
    padding: '8px', borderRadius: '6px', backgroundColor: '#f3f4f6', textAlign: 'center',
  },
  statNum: { fontSize: '20px', fontWeight: 700, color: '#4f46e5' },
  statLabel: { fontSize: '11px', color: '#6b7280' },
  row: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' },
  toggle: (on) => ({
    width: '36px', height: '20px', borderRadius: '10px', border: 'none', cursor: 'pointer',
    backgroundColor: on ? '#4f46e5' : '#d1d5db', position: 'relative', transition: 'background-color .2s',
  }),
  toggleDot: (on) => ({
    width: '16px', height: '16px', borderRadius: '50%', backgroundColor: '#fff',
    position: 'absolute', top: '2px', left: on ? '18px' : '2px', transition: 'left .2s',
  }),
  btn: {
    width: '100%', padding: '8px', border: 'none', borderRadius: '6px', cursor: 'pointer',
    backgroundColor: '#4f46e5', color: '#fff', fontSize: '13px', marginTop: '4px',
  },
  btnSecondary: {
    width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px',
    cursor: 'pointer', backgroundColor: '#fff', fontSize: '13px', marginTop: '4px',
  },
};

function Popup() {
  const [online, setOnline] = useState(false);
  const [stats, setStats] = useState({ items: 0, vectors: 0, pending: 0 });
  const [injecting, setInjecting] = useState(true);
  const [loading, setLoading] = useState(true);
  // G5 (W3 batch B): tab 切换 — 'main' 默认；'privacy' 显示浏览隐私面板
  // per R02 P0-1：之前 Privacy.jsx 完全死代码，用户没有 UI 入口
  const [tab, setTab] = useState('main');

  useEffect(() => {
    sendToWorker(MSG.GET_STATUS).then((res) => {
      if (res) {
        setOnline(res.online);
        setStats({
          items: res.total_items || 0,
          vectors: res.total_vectors || 0,
          pending: res.pending_embeddings || 0,
        });
        setInjecting(res.injection_enabled !== false);
      }
      setLoading(false);
    }).catch(() => { setOnline(false); setLoading(false); });
  }, []);

  const toggleInjection = () => {
    const next = !injecting;
    setInjecting(next);
    sendToWorker(MSG.TOGGLE_INJECTION, { enabled: next });
  };

  const openPanel = () => sendToWorker(MSG.OPEN_SIDEPANEL);
  const openOptions = () => chrome.runtime.openOptionsPage();

  // G5 tab 路由：privacy tab 显示 Privacy 组件
  if (tab === 'privacy') {
    return (
      <div>
        <div style={{ padding: '8px 12px', borderBottom: '1px solid #e5e7eb', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <button
            onClick={() => setTab('main')}
            style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: '14px', color: '#4f46e5' }}
          >← 返回</button>
        </div>
        <Privacy />
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.dot(online)} />
        <h2 style={styles.title}>npu-webhook</h2>
      </div>

      {!online && !loading && (
        <div style={{ fontSize: '12px', color: '#dc2626', marginBottom: '8px', padding: '6px 8px', background: '#fef2f2', borderRadius: '4px' }}>
          后端离线，请启动 npu-webhook
        </div>
      )}

      <div style={styles.stats}>
        <div style={styles.stat}>
          <div style={styles.statNum}>{loading ? '…' : stats.items}</div>
          <div style={styles.statLabel}>知识条目</div>
        </div>
        <div style={styles.stat}>
          <div style={styles.statNum}>{loading ? '…' : stats.vectors}</div>
          <div style={styles.statLabel}>向量数</div>
        </div>
      </div>
      {stats.pending > 0 && (
        <div style={{ fontSize: '11px', color: '#9ca3af', textAlign: 'center', marginBottom: '8px' }}>
          {stats.pending} 条待 embedding 处理
        </div>
      )}

      <div style={styles.row}>
        <span>知识注入</span>
        <button style={styles.toggle(injecting)} onClick={toggleInjection}>
          <span style={styles.toggleDot(injecting)} />
        </button>
      </div>

      <button style={styles.btn} onClick={openPanel}>打开知识面板</button>
      <button style={styles.btnSecondary} onClick={openOptions}>设置</button>
      {/* G5 入口（W3 batch B, per R02 P0-1） */}
      <button style={styles.btnSecondary} onClick={() => setTab('privacy')}>浏览隐私 →</button>
    </div>
  );
}

render(<Popup />, document.getElementById('app'));
