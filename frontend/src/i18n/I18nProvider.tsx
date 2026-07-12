'use client';

import { useEffect, useState } from 'react';
import { I18nextProvider } from 'react-i18next';

import i18n, { initUiLanguage } from './index';

/**
 * Applies the persisted (or OS-autodetected) UI language before rendering children.
 *
 * We hold the first paint until the language is resolved, otherwise a Chinese user
 * sees a flash of English on every launch.
 */
export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    initUiLanguage()
      .catch(() => {
        // Detection failing must never block the app — fall back to the default.
      })
      .finally(() => {
        if (!cancelled) setReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!ready) return null;

  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}
