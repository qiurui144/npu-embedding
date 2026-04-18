/** Wizard Step 1 · 欢迎页 */

import type { JSX } from 'preact';
import { Button } from '../components';
import { t } from '../i18n';

type PillarProps = {
  icon: string;
  title: string;
  desc: string;
};

function Pillar({ icon, title, desc }: PillarProps): JSX.Element {
  return (
    <div
      style={{
        flex: 1,
        padding: 'var(--space-4)',
        background: 'var(--color-bg)',
        borderRadius: 'var(--radius-lg)',
        border: '1px solid var(--color-border)',
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--space-2)',
        alignItems: 'center',
        textAlign: 'center',
      }}
    >
      <div aria-hidden="true" style={{ fontSize: 28 }}>
        {icon}
      </div>
      <h4
        style={{
          fontSize: 'var(--text-base)',
          fontWeight: 600,
          color: 'var(--color-text)',
          margin: 0,
        }}
      >
        {title}
      </h4>
      <p
        style={{
          fontSize: 'var(--text-xs)',
          color: 'var(--color-text-secondary)',
          margin: 0,
          lineHeight: 'var(--leading-normal)',
        }}
      >
        {desc}
      </p>
    </div>
  );
}

export type Step1Props = {
  onContinue: () => void;
  onImport: () => void;
};

export function Step1Welcome({ onContinue, onImport }: Step1Props): JSX.Element {
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 'var(--space-5)',
        alignItems: 'center',
        textAlign: 'center',
      }}
    >
      <div style={{ fontSize: 48 }} aria-hidden="true">
        🌿
      </div>
      <div>
        <h1
          style={{
            fontSize: 'var(--text-2xl)',
            fontWeight: 700,
            color: 'var(--color-text)',
            margin: 0,
            marginBottom: 'var(--space-2)',
          }}
        >
          {t('wizard.welcome.title')}
        </h1>
        <p
          style={{
            fontSize: 'var(--text-lg)',
            color: 'var(--color-text-secondary)',
            margin: 0,
            marginBottom: 'var(--space-3)',
          }}
        >
          {t('wizard.welcome.sub')}
        </p>
        <p
          style={{
            fontSize: 'var(--text-sm)',
            color: 'var(--color-text-secondary)',
            margin: 0,
            maxWidth: 480,
          }}
        >
          {t('app.promise')}
        </p>
      </div>

      {/* 3 支柱 */}
      <div
        style={{
          display: 'flex',
          gap: 'var(--space-3)',
          width: '100%',
          marginTop: 'var(--space-3)',
        }}
      >
        <Pillar
          icon="🌱"
          title={t('wizard.welcome.pillar.evolve')}
          desc={t('wizard.welcome.pillar.evolve_desc')}
        />
        <Pillar
          icon="💬"
          title={t('wizard.welcome.pillar.companion')}
          desc={t('wizard.welcome.pillar.companion_desc')}
        />
        <Pillar
          icon="🔄"
          title={t('wizard.welcome.pillar.hybrid')}
          desc={t('wizard.welcome.pillar.hybrid_desc')}
        />
      </div>

      {/* CTA */}
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--space-2)',
          marginTop: 'var(--space-4)',
          width: '100%',
          maxWidth: 320,
        }}
      >
        <Button variant="primary" size="lg" fullWidth onClick={onContinue}>
          {t('wizard.welcome.cta')}
        </Button>
        <Button variant="ghost" size="md" fullWidth onClick={onImport}>
          {t('wizard.welcome.import_existing')}
        </Button>
      </div>
    </div>
  );
}
