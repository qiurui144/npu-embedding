/** Wizard Step 5 · 第一口知识 · 三选一 */

import type { JSX } from 'preact';
import { useState, useRef } from 'preact/hooks';
import { Button } from '../components';
import { toast } from '../components/Toast';
import { t } from '../i18n';
import { api } from '../store/api';
import type { WizardContext } from './types';

type DataMode = 'folder' | 'import' | 'skip';

export type Step5Props = {
  ctx: WizardContext;
  onUpdate: (partial: Partial<WizardContext>) => void;
  onFinish: () => void;
};

export function Step5Data({ ctx, onUpdate, onFinish }: Step5Props): JSX.Element {
  const [mode, setMode] = useState<DataMode | null>(ctx.dataMode);
  const [folderPath, setFolderPath] = useState('');
  const [importing, setImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  async function handleFinish() {
    if (!mode) {
      toast('warning', '请选择一个选项');
      return;
    }
    onUpdate({ dataMode: mode });
    setImporting(true);

    try {
      if (mode === 'folder' && folderPath) {
        await api.post('/index/bind', { path: folderPath, recursive: true });
        onUpdate({ boundFolder: folderPath });
        toast('success', '文件夹已绑定，后台开始索引');
      } else if (mode === 'import') {
        const file = fileInputRef.current?.files?.[0];
        if (file) {
          const text = await file.text();
          const profile = JSON.parse(text);
          await api.post('/profile/import', profile);
          onUpdate({ importedProfile: file.name });
          toast('success', `已导入 ${file.name}`);
        }
      }
      onFinish();
    } catch (e) {
      toast('error', e instanceof Error ? e.message : String(e));
      setImporting(false);
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }}>
      <h2 style={{ fontSize: 'var(--text-xl)', fontWeight: 600, margin: 0 }}>
        {t('wizard.data.heading')}
      </h2>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr 1fr',
          gap: 'var(--space-3)',
        }}
      >
        {/* 绑定文件夹 */}
        <Option
          icon="📂"
          title={t('wizard.data.folder.title')}
          desc={t('wizard.data.folder.desc')}
          selected={mode === 'folder'}
          onClick={() => setMode('folder')}
        >
          {mode === 'folder' && (
            <input
              type="text"
              placeholder="/home/user/Documents/knowledge"
              value={folderPath}
              onInput={(e) => setFolderPath(e.currentTarget.value)}
              onClick={(e) => e.stopPropagation()}
              style={{
                width: '100%',
                padding: 'var(--space-2)',
                fontSize: 'var(--text-xs)',
                fontFamily: 'var(--font-mono)',
                border: '1px solid var(--color-border)',
                borderRadius: 'var(--radius-sm)',
                marginTop: 'var(--space-2)',
                background: 'var(--color-surface)',
              }}
            />
          )}
        </Option>

        {/* 导入 profile */}
        <Option
          icon="📥"
          title={t('wizard.data.import.title')}
          desc={t('wizard.data.import.desc')}
          selected={mode === 'import'}
          onClick={() => {
            setMode('import');
            fileInputRef.current?.click();
          }}
        >
          <>
            <input
              ref={fileInputRef}
              type="file"
              accept=".json,.vault-profile"
              style={{ display: 'none' }}
              onClick={(e) => e.stopPropagation()}
            />
            {mode === 'import' && fileInputRef.current?.files?.[0] && (
              <div
                style={{
                  marginTop: 'var(--space-2)',
                  fontSize: 'var(--text-xs)',
                  color: 'var(--color-accent)',
                }}
              >
                ✓ {fileInputRef.current.files[0].name}
              </div>
            )}
          </>
        </Option>

        {/* 跳过 */}
        <Option
          icon="→"
          title={t('wizard.data.skip.title')}
          desc={t('wizard.data.skip.desc')}
          selected={mode === 'skip'}
          onClick={() => setMode('skip')}
        />
      </div>

      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        <Button
          variant="primary"
          size="lg"
          loading={importing}
          disabled={!mode}
          onClick={handleFinish}
        >
          {t('wizard.data.finish')} →
        </Button>
      </div>
    </div>
  );
}

function Option({
  icon,
  title,
  desc,
  selected,
  onClick,
  children,
}: {
  icon: string;
  title: string;
  desc: string;
  selected: boolean;
  onClick: () => void;
  children?: JSX.Element | JSX.Element[] | false | null;
}): JSX.Element {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={selected}
      className="interactive"
      style={{
        padding: 'var(--space-4)',
        background: 'var(--color-surface)',
        border: `2px solid ${selected ? 'var(--color-accent)' : 'var(--color-border)'}`,
        borderRadius: 'var(--radius-lg)',
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--space-2)',
        textAlign: 'left',
        cursor: 'pointer',
        minHeight: 160,
      }}
    >
      <div style={{ fontSize: 24 }} aria-hidden="true">
        {icon}
      </div>
      <h3 style={{ fontSize: 'var(--text-base)', fontWeight: 600, margin: 0 }}>
        {title}
      </h3>
      <p
        style={{
          fontSize: 'var(--text-xs)',
          color: 'var(--color-text-secondary)',
          margin: 0,
          lineHeight: 1.5,
        }}
      >
        {desc}
      </p>
      {children}
    </button>
  );
}
