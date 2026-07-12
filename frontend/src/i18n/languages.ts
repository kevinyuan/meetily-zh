/**
 * UI languages shipped with the app.
 *
 * To add a language:
 *   1. create `src/i18n/locales/<code>/translation.json`
 *   2. add an entry here
 *   3. register the import in `src/i18n/index.ts` (webpack needs static imports)
 */

export interface LanguageMeta {
  /** English name, e.g. "Simplified Chinese" */
  name: string;
  /** Endonym, shown to the user, e.g. "简体中文" */
  nativeName: string;
  /** Lower sorts first in the picker. */
  priority: number;
}

export const LANGUAGE_METADATA: Record<string, LanguageMeta> = {
  en: { name: 'English', nativeName: 'English', priority: 1 },
  'zh-Hans': { name: 'Simplified Chinese', nativeName: '简体中文', priority: 2 },
};

export const DEFAULT_UI_LANGUAGE = 'en';

export const SUPPORTED_UI_LANGUAGES = Object.keys(LANGUAGE_METADATA).sort(
  (a, b) => LANGUAGE_METADATA[a].priority - LANGUAGE_METADATA[b].priority,
);

/**
 * Sentinel stored by the *other* language preferences (model filter, transcription,
 * summary) to mean "whatever the UI language currently is". It is deliberately never
 * materialised into a concrete code, so changing the UI language re-resolves all of
 * them instead of leaving stale copies behind.
 */
export const FOLLOW_UI_LANGUAGE = 'follow-ui';

/**
 * Map an arbitrary BCP-47 tag (an OS locale, or a stored preference) onto a UI
 * language we actually ship. Falls back to English.
 *
 * Handles the cases macOS/Windows actually emit: `zh-Hans`, `zh-Hans-CN`, `zh-CN`,
 * `zh-SG` are Simplified; `zh-Hant`, `zh-TW`, `zh-HK` are Traditional and, since we
 * do not ship a Traditional locale yet, fall back to English rather than silently
 * showing Simplified.
 */
export function resolveUiLanguage(tag: string | null | undefined): string {
  if (!tag) return DEFAULT_UI_LANGUAGE;

  if (LANGUAGE_METADATA[tag]) return tag;

  const lower = tag.toLowerCase();

  if (lower.startsWith('zh')) {
    const isTraditional =
      lower.includes('hant') || lower.includes('tw') || lower.includes('hk') || lower.includes('mo');
    return isTraditional ? DEFAULT_UI_LANGUAGE : 'zh-Hans';
  }

  const base = lower.split('-')[0];
  const match = SUPPORTED_UI_LANGUAGES.find((code) => code.toLowerCase().split('-')[0] === base);
  return match ?? DEFAULT_UI_LANGUAGE;
}

/**
 * The language code the speech/summary engines should use for a given UI language.
 * Engines want a plain ISO-639-1 code (`zh`), not a script-qualified tag (`zh-Hans`).
 */
export function uiLanguageToEngineCode(uiLanguage: string): string {
  return uiLanguage.split('-')[0];
}
