import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './locales/en/translation.json';
import zhHans from './locales/zh-Hans/translation.json';
import { DEFAULT_UI_LANGUAGE, resolveUiLanguage, SUPPORTED_UI_LANGUAGES } from './languages';

export const UI_LANGUAGE_STORAGE_KEY = 'uiLanguage';

/**
 * Locale resources. Webpack needs static imports, so each new locale must be
 * registered here by hand (Vite's `import.meta.glob` is not available to us).
 */
const resources = {
  en: { translation: en },
  'zh-Hans': { translation: zhHans },
} as const;

i18n.use(initReactI18next).init({
  resources,
  lng: DEFAULT_UI_LANGUAGE,
  fallbackLng: DEFAULT_UI_LANGUAGE,
  supportedLngs: SUPPORTED_UI_LANGUAGES,
  interpolation: { escapeValue: false },
  // No SSR in this app (static export + 'use client' everywhere), and suspense
  // would otherwise blank the tree on first paint.
  react: { useSuspense: false },
});

/**
 * Resolve the UI language to use on boot: an explicit user choice wins, otherwise
 * autodetect from the OS locale.
 *
 * Safe to call before the Tauri runtime exists — falls back to the browser locale,
 * which is what `next dev` in a plain browser will hit.
 */
async function detectInitialLanguage(): Promise<string> {
  const stored = window.localStorage.getItem(UI_LANGUAGE_STORAGE_KEY);
  if (stored && SUPPORTED_UI_LANGUAGES.includes(stored)) return stored;

  try {
    const { locale } = await import('@tauri-apps/plugin-os');
    const osLocale = await locale();
    if (osLocale) return resolveUiLanguage(osLocale);
  } catch {
    // Not running under Tauri (or the OS plugin is unavailable) — fall through.
  }

  return resolveUiLanguage(window.navigator?.language);
}

/** Persist and apply a UI language chosen by the user. */
export async function setUiLanguage(language: string): Promise<void> {
  window.localStorage.setItem(UI_LANGUAGE_STORAGE_KEY, language);
  await i18n.changeLanguage(language);
}

/**
 * Called once from the client-side i18n provider. Kept out of module scope because
 * `next build` prerenders these files in Node, where `window` does not exist.
 */
export async function initUiLanguage(): Promise<void> {
  if (typeof window === 'undefined') return;
  const language = await detectInitialLanguage();
  if (language !== i18n.language) {
    await i18n.changeLanguage(language);
  }
}

if (typeof document !== 'undefined') {
  i18n.on('languageChanged', (language) => {
    document.documentElement.lang = language;
  });
}

export default i18n;
