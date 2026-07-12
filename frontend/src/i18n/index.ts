import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import { DEFAULT_UI_LANGUAGE, resolveUiLanguage, SUPPORTED_UI_LANGUAGES } from './languages';

// Locale resources are split by area so they can be worked on independently.
// Webpack needs static imports (Vite's import.meta.glob is not available to us), so
// each new area file must be registered here by hand.
import enCore from './locales/en/translation.json';
import enSettings from './locales/en/settings.json';
import enRecording from './locales/en/recording.json';
import enMeeting from './locales/en/meeting.json';
import enOnboarding from './locales/en/onboarding.json';
import enDialogs from './locales/en/dialogs.json';
import enModels from './locales/en/models.json';

import zhCore from './locales/zh-Hans/translation.json';
import zhSettings from './locales/zh-Hans/settings.json';
import zhRecording from './locales/zh-Hans/recording.json';
import zhMeeting from './locales/zh-Hans/meeting.json';
import zhOnboarding from './locales/zh-Hans/onboarding.json';
import zhDialogs from './locales/zh-Hans/dialogs.json';
import zhModels from './locales/zh-Hans/models.json';

export const UI_LANGUAGE_STORAGE_KEY = 'uiLanguage';

/**
 * Area files own disjoint top-level keys, so a shallow merge is sufficient and a
 * collision is a bug worth failing loudly on in development.
 */
function mergeAreas(...areas: Record<string, unknown>[]): Record<string, unknown> {
  const merged: Record<string, unknown> = {};
  for (const area of areas) {
    for (const [key, value] of Object.entries(area)) {
      if (process.env.NODE_ENV !== 'production' && key in merged) {
        console.warn(`[i18n] duplicate top-level key "${key}" across locale area files`);
      }
      merged[key] = value;
    }
  }
  return merged;
}

const resources = {
  en: {
    translation: mergeAreas(
      enCore,
      enSettings,
      enRecording,
      enMeeting,
      enOnboarding,
      enDialogs,
      enModels,
    ),
  },
  'zh-Hans': {
    translation: mergeAreas(
      zhCore,
      zhSettings,
      zhRecording,
      zhMeeting,
      zhOnboarding,
      zhDialogs,
      zhModels,
    ),
  },
};

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
