'use client';

import { useEffect } from 'react';
import { I18nextProvider } from 'react-i18next';

import i18n, { initUiLanguage } from './index';

/**
 * Makes the i18n instance available to the tree and kicks off language detection.
 *
 * It deliberately renders its children immediately. An earlier version withheld them
 * until detection finished ("otherwise a Chinese user sees a flash of English"), which
 * blanked the ENTIRE app the moment detection failed to settle — and it could not
 * settle, because OS-locale detection calls a Tauri plugin that is not registered on
 * the Rust side, so its IPC promise never resolved. No UI may hang on a promise it
 * does not control.
 *
 * The flash it was guarding against is handled properly instead: an explicit language
 * choice is read synchronously from localStorage before i18n initialises, so the first
 * paint is already in the right language. Only first-run OS autodetection can change
 * the language after mount, and react-i18next re-renders on `languageChanged`.
 */
export function I18nProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    initUiLanguage().catch((error) => {
      // Detection failing must never take the app down with it.
      console.error('[i18n] Language detection failed; keeping the current language:', error);
    });
  }, []);

  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}
