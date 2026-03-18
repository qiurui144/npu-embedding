import { h, render } from 'preact';
import { useState, useEffect } from 'preact/hooks';
import { getSettings, saveSettings } from '../shared/storage.js';

const styles = {
  page: { padding: '24px', maxWidth: '600px', margin: '0 auto', fontFamily: 'system-ui, sans-serif' },
  title: { fontSize: '22px', fontWeight: 700, marginBottom: '24px' },
  field: { marginBottom: '16px' },
  label: { display: 'block', fontWeight: 600, marginBottom: '4px', fontSize: '14px' },
  hint: { fontSize: '12px', color: '#6b7280', marginBottom: '4px' },
  input: {
    width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px',
    fontSize: '14px', boxSizing: 'border-box',
  },
  textarea: {
    width: '100%', padding: '8px', border: '1px solid #d1d5db', borderRadius: '6px',
    fontSize: '14px', boxSizing: 'border-box', minHeight: '80px', resize: 'vertical',
  },
  radioGroup: { display: 'flex', gap: '16px', marginTop: '4px' },
  radioLabel: { display: 'flex', alignItems: 'center', gap: '4px', cursor: 'pointer', fontSize: '14px' },
  btnRow: { display: 'flex', gap: '8px', marginTop: '20px' },
  btn: {
    padding: '8px 20px', border: 'none', borderRadius: '6px', cursor: 'pointer',
    backgroundColor: '#4f46e5', color: '#fff', fontSize: '14px',
  },
  btnSecondary: {
    padding: '8px 20px', border: '1px solid #d1d5db', borderRadius: '6px',
    cursor: 'pointer', backgroundColor: '#fff', fontSize: '14px',
  },
  toast: (ok) => ({
    padding: '8px 16px', borderRadius: '6px',
    backgroundColor: ok ? '#dcfce7' : '#fee2e2',
    color: ok ? '#166534' : '#991b1b', marginTop: '12px', fontSize: '13px',
  }),
};

function Options() {
  const [backendUrl, setBackendUrl] = useState('http://localhost:18900');
  const [injectionMode, setInjectionMode] = useState('auto');
  const [excludedDomains, setExcludedDomains] = useState('');
  const [toast, setToast] = useState(null);

  useEffect(() => {
    getSettings().then((s) => {
      setBackendUrl(s.backendUrl || 'http://localhost:18900');
      setInjectionMode(s.injectionMode || 'auto');
      setExcludedDomains((s.excludedDomains || []).join('\n'));
    });
  }, []);

  const save = async () => {
    const settings = {
      backendUrl: backendUrl.replace(/\/+$/, ''),
      injectionMode,
      excludedDomains: excludedDomains.split('\n').map((d) => d.trim()).filter(Boolean),
    };
    await saveSettings(settings);
    setToast({ ok: true, msg: '设置已保存' });
    setTimeout(() => setToast(null), 2000);
  };

  const testConnection = async () => {
    try {
      const resp = await fetch(`${backendUrl.replace(/\/+$/, '')}/api/v1/status/health`);
      if (resp.ok) {
        setToast({ ok: true, msg: '连接成功' });
      } else {
        setToast({ ok: false, msg: `连接失败: HTTP ${resp.status}` });
      }
    } catch (e) {
      setToast({ ok: false, msg: `连接失败: ${e.message}` });
    }
    setTimeout(() => setToast(null), 3000);
  };

  return (
    <div style={styles.page}>
      <h1 style={styles.title}>npu-webhook 设置</h1>

      <div style={styles.field}>
        <label style={styles.label}>后端地址</label>
        <div style={styles.hint}>知识库服务运行的地址</div>
        <input
          style={styles.input} value={backendUrl}
          onInput={(e) => setBackendUrl(e.target.value)}
          placeholder="http://localhost:18900"
        />
      </div>

      <div style={styles.field}>
        <label style={styles.label}>注入模式</label>
        <div style={styles.radioGroup}>
          {[['auto', '自动'], ['manual', '手动'], ['disabled', '禁用']].map(([val, label]) => (
            <label key={val} style={styles.radioLabel}>
              <input
                type="radio" name="mode" value={val}
                checked={injectionMode === val}
                onChange={() => setInjectionMode(val)}
              />
              {label}
            </label>
          ))}
        </div>
      </div>

      <div style={styles.field}>
        <label style={styles.label}>排除域名</label>
        <div style={styles.hint}>每行一个域名，在这些网站上不会自动注入</div>
        <textarea
          style={styles.textarea} value={excludedDomains}
          onInput={(e) => setExcludedDomains(e.target.value)}
          placeholder="example.com"
        />
      </div>

      <div style={styles.btnRow}>
        <button style={styles.btn} onClick={save}>保存</button>
        <button style={styles.btnSecondary} onClick={testConnection}>测试连接</button>
      </div>

      {toast && <div style={styles.toast(toast.ok)}>{toast.msg}</div>}
    </div>
  );
}

render(<Options />, document.getElementById('app'));
