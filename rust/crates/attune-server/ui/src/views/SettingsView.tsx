/** Settings 视图 · C 方案（左 tab + 右内容面板）
 * 见 spec §4 "关于 Settings · C 混合方案"
 */

import type { JSX } from 'preact';
import { useSignal } from '@preact/signals';

type SettingsTab = 'general' | 'ai' | 'data' | 'privacy' | 'about';

const TABS: Array<{ key: SettingsTab; icon: string; label: string }> = [
  { key: 'general', icon: '⚙', label: '通用' },
  { key: 'ai', icon: '🤖', label: 'AI 大脑' },
  { key: 'data', icon: '📂', label: '数据' },
  { key: 'privacy', icon: '🔐', label: '隐私' },
  { key: 'about', icon: 'ℹ', label: '关于' },
];

export function SettingsView(): JSX.Element {
  const activeTab = useSignal<SettingsTab>('general');

  return (
    <div
      style={{
        height: '100%',
        display: 'flex',
      }}
    >
      {/* 左 tab 栏 */}
      <nav
        aria-label="Settings sections"
        style={{
          width: 200,
          flexShrink: 0,
          borderRight: '1px solid var(--color-border)',
          padding: 'var(--space-5) 0',
          background: 'var(--color-bg)',
        }}
      >
        <h2
          style={{
            fontSize: 'var(--text-xl)',
            fontWeight: 600,
            margin: 0,
            padding: '0 var(--space-5)',
            marginBottom: 'var(--space-4)',
          }}
        >
          设置
        </h2>
        {TABS.map((tab) => {
          const active = activeTab.value === tab.key;
          return (
            <button
              key={tab.key}
              type="button"
              onClick={() => (activeTab.value = tab.key)}
              aria-current={active ? 'page' : undefined}
              className="interactive"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-3)',
                width: '100%',
                padding: 'var(--space-2) var(--space-5)',
                background: active ? 'var(--color-surface-hover)' : 'transparent',
                borderLeft: `2px solid ${active ? 'var(--color-accent)' : 'transparent'}`,
                border: 'none',
                borderLeftWidth: 2,
                borderLeftStyle: 'solid',
                borderLeftColor: active ? 'var(--color-accent)' : 'transparent',
                color: active ? 'var(--color-text)' : 'var(--color-text-secondary)',
                fontSize: 'var(--text-sm)',
                textAlign: 'left',
                cursor: 'pointer',
              }}
            >
              <span aria-hidden="true">{tab.icon}</span>
              <span>{tab.label}</span>
            </button>
          );
        })}
      </nav>

      {/* 右 内容面板 */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          padding: 'var(--space-6) var(--space-7)',
        }}
      >
        {activeTab.value === 'general' && <GeneralPanel />}
        {activeTab.value === 'ai' && <AIPanel />}
        {activeTab.value === 'data' && <DataPanel />}
        {activeTab.value === 'privacy' && <PrivacyPanel />}
        {activeTab.value === 'about' && <AboutPanel />}
      </div>
    </div>
  );
}

// ── Panel 占位 ─────────────────────────────────────────────
function Section({
  title,
  children,
}: {
  title: string;
  children: JSX.Element | JSX.Element[];
}): JSX.Element {
  return (
    <section style={{ marginBottom: 'var(--space-6)' }}>
      <h3
        style={{
          fontSize: 'var(--text-lg)',
          fontWeight: 600,
          margin: 0,
          marginBottom: 'var(--space-3)',
        }}
      >
        {title}
      </h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }}>
        {children}
      </div>
    </section>
  );
}

function GeneralPanel(): JSX.Element {
  return (
    <>
      <Section title="通用">
        <SettingRow label="主题" value="跟随系统" />
        <SettingRow label="语言" value="中文" />
      </Section>
      <Section title="界面">
        <SettingRow label="Sidebar 默认折叠" value="否" />
        <SettingRow label="减少动效" value="跟随系统" />
      </Section>
    </>
  );
}

function AIPanel(): JSX.Element {
  return (
    <>
      <Section title="AI 大脑">
        <SettingRow label="后端" value="本地 Ollama" />
        <SettingRow label="Chat 模型" value="qwen2.5:3b" />
        <SettingRow label="Embedding" value="bge-m3" />
        <SettingRow label="摘要模型" value="qwen2.5:3b" />
      </Section>
      <Section title="网络搜索">
        <SettingRow label="启用" value="是（浏览器自动化）" />
        <SettingRow label="引擎" value="DuckDuckGo" />
      </Section>
    </>
  );
}

function DataPanel(): JSX.Element {
  return (
    <>
      <Section title="数据源">
        <SettingRow label="已绑定文件夹" value="0" />
        <SettingRow label="远程目录" value="0" />
      </Section>
      <Section title="备份">
        <SettingRow label="自动备份" value="每日 03:00" />
        <SettingRow label="保留策略" value="7 日 + 4 周" />
      </Section>
    </>
  );
}

function PrivacyPanel(): JSX.Element {
  return (
    <>
      <Section title="安全">
        <SettingRow label="Vault 状态" value="已解锁" />
        <SettingRow label="Device Secret" value="—" />
      </Section>
      <Section title="遥测">
        <SettingRow label="匿名使用统计" value="关闭（默认）" />
      </Section>
    </>
  );
}

function AboutPanel(): JSX.Element {
  return (
    <>
      <Section title="Attune">
        <p style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)', margin: 0 }}>
          私有 AI 知识伙伴 · 本地决定，全网增强，越用越懂你的专业。
        </p>
        <SettingRow label="版本" value="0.6.0-dev" />
        <SettingRow label="许可" value="Apache-2.0" />
      </Section>
    </>
  );
}

function SettingRow({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: 'var(--space-3) var(--space-4)',
        background: 'var(--color-surface)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius-md)',
      }}
    >
      <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text)' }}>{label}</span>
      <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-text-secondary)' }}>
        {value}
      </span>
    </div>
  );
}
