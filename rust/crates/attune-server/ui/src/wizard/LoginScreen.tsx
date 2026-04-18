/** Login Screen · vault 已 setup 但 locked 时显示 · 输入 master password 解锁 */

import type { JSX } from 'preact';
import { useState } from 'preact/hooks';
import { Button, Input } from '../components';
import { toast } from '../components/Toast';
import { t } from '../i18n';
import { api, setToken } from '../store/api';

export type LoginScreenProps = {
  onUnlock: () => void;
};

export function LoginScreen({ onUnlock }: LoginScreenProps): JSX.Element {
  const [pwd, setPwd] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleUnlock(e?: Event) {
    e?.preventDefault();
    if (!pwd) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await api.post<{ status: string; token?: string }>(
        '/vault/unlock',
        { password: pwd },
      );
      if (res.token) setToken(res.token);
      toast('success', '已解锁');
      onUnlock();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setSubmitting(false);
    }
  }

  return (
    <div
      style={{
        minHeight: '100vh',
        background:
          'radial-gradient(ellipse at top right, #E9EEF2 0%, #F7F8FA 50%)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 'var(--space-5)',
      }}
    >
      <form
        onSubmit={handleUnlock}
        className="fade-slide-in"
        style={{
          background: 'var(--color-surface)',
          borderRadius: 'var(--radius-xl)',
          boxShadow: 'var(--shadow-lg)',
          padding: 'var(--space-7) var(--space-6)',
          maxWidth: 400,
          width: '100%',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--space-5)',
          alignItems: 'center',
        }}
      >
        <div style={{ fontSize: 48 }} aria-hidden="true">
          🔒
        </div>
        <div style={{ textAlign: 'center' }}>
          <h1
            style={{
              fontSize: 'var(--text-xl)',
              fontWeight: 600,
              margin: 0,
              marginBottom: 'var(--space-2)',
            }}
          >
            {t('app.name')}
          </h1>
          <p
            style={{
              fontSize: 'var(--text-sm)',
              color: 'var(--color-text-secondary)',
              margin: 0,
            }}
          >
            Vault 已锁定 · 请输入 Master Password
          </p>
        </div>

        <div style={{ width: '100%' }}>
          <Input
            type="password"
            value={pwd}
            onInput={(e) => setPwd(e.currentTarget.value)}
            error={error ?? undefined}
            autoFocus
            required
            aria-label="Master Password"
            placeholder="••••••••••••"
          />
        </div>

        <Button
          type="submit"
          variant="primary"
          size="lg"
          fullWidth
          loading={submitting}
          disabled={!pwd}
          onClick={() => handleUnlock()}
        >
          解锁
        </Button>

        <p
          style={{
            fontSize: 'var(--text-xs)',
            color: 'var(--color-text-secondary)',
            textAlign: 'center',
            margin: 0,
          }}
        >
          忘记密码无法找回。密码本身从未离开你的设备。
        </p>
      </form>
    </div>
  );
}
