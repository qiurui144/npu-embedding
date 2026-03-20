// extension/src/sidepanel/pages/FilePage.jsx
import { h } from 'preact';
import { useState, useRef } from 'preact/hooks';
import { api } from '../../shared/api.js';

const ALLOWED_TYPES = ['.pdf', '.docx', '.md', '.txt', '.py', '.js', '.ts'];
const SESSION_ID_KEY = 'npu_session_id';

function getSessionId() {
  let sid = sessionStorage.getItem(SESSION_ID_KEY);
  if (!sid) {
    sid = Math.random().toString(36).slice(2);
    sessionStorage.setItem(SESSION_ID_KEY, sid);
  }
  return sid;
}

export default function FilePage() {
  const [files, setFiles] = useState([]);   // [{name, id, status, chunks}]
  const [dragging, setDragging] = useState(false);
  const [uploading, setUploading] = useState(false);
  const inputRef = useRef(null);

  async function uploadFile(file) {
    const ext = '.' + file.name.split('.').pop().toLowerCase();
    if (!ALLOWED_TYPES.includes(ext)) {
      alert(`不支持的格式：${ext}。支持：${ALLOWED_TYPES.join(' ')}`);
      return;
    }
    setUploading(true);
    setFiles((prev) => [...prev, { name: file.name, id: null, status: 'uploading', chunks: 0 }]);
    try {
      const result = await api.uploadFile(file, getSessionId());
      setFiles((prev) =>
        prev.map((f) =>
          f.name === file.name && f.status === 'uploading'
            ? { ...f, id: result.id, status: 'done', chunks: result.chunks_queued }
            : f,
        ),
      );
    } catch (err) {
      setFiles((prev) =>
        prev.map((f) =>
          f.name === file.name && f.status === 'uploading' ? { ...f, status: 'error' } : f,
        ),
      );
    } finally {
      setUploading(false);
    }
  }

  function handleDrop(e) {
    e.preventDefault();
    setDragging(false);
    Array.from(e.dataTransfer.files).forEach(uploadFile);
  }

  async function handleDelete(id) {
    if (!id) return;
    try {
      await api.deleteItem(id);
      setFiles((prev) => prev.filter((f) => f.id !== id));
    } catch { /* ignore */ }
  }

  return (
    <div class="fp-container">
      <div
        class={`fp-dropzone${dragging ? ' fp-dropzone--active' : ''}`}
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
        onClick={() => inputRef.current?.click()}
      >
        <span>拖拽文件到此处，或点击选择</span>
        <span class="fp-hint">支持：PDF DOCX MD TXT Python JS TS</span>
        <input
          ref={inputRef}
          type="file"
          accept={ALLOWED_TYPES.join(',')}
          multiple
          style="display:none"
          onChange={(e) => Array.from(e.target.files).forEach(uploadFile)}
        />
      </div>

      {files.length > 0 && (
        <ul class="fp-list">
          {files.map((f, i) => (
            <li key={i} class="fp-item">
              <span class={`fp-status fp-status--${f.status}`}>
                {f.status === 'uploading' ? '上传中...' : f.status === 'done' ? '✓' : '✗'}
              </span>
              <span class="fp-name">{f.name}</span>
              {f.status === 'done' && (
                <span class="fp-meta">已处理（{f.chunks} 个段落）</span>
              )}
              {f.id && (
                <button class="fp-btn-delete" onClick={() => handleDelete(f.id)}>删除</button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
