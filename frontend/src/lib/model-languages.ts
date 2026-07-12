import { FOLLOW_UI_LANGUAGE, uiLanguageToEngineCode } from '@/i18n/languages';

/**
 * Which languages each transcription provider can actually handle.
 *
 * Mirrors the Rust catalog (`config.rs`: WHISPER_LANGUAGES / PARAKEET_LANGUAGES,
 * `sensevoice_engine.rs`: SENSEVOICE_LANGUAGES). Kept in sync deliberately: the
 * backend is authoritative and each model's `languages` field is returned with the
 * model list — this map is only used to filter the *provider* dropdown, before any
 * model list has been fetched.
 *
 * Parakeet is English-only. Its v3 model adds European languages but NOT Chinese,
 * so it must never gain "zh" here.
 */
export const PROVIDER_LANGUAGES: Record<string, string[]> = {
  localWhisper: ['en', 'zh'],
  parakeet: ['en'],
  senseVoice: ['zh', 'en', 'ja', 'ko', 'yue'],
};

export const ALL_LANGUAGES = 'all';

/** The value stored in the model-language filter. */
export type LanguageFilter = typeof ALL_LANGUAGES | typeof FOLLOW_UI_LANGUAGE | string;

export const MODEL_FILTER_STORAGE_KEY = 'modelLanguageFilter';

/**
 * Resolve a stored filter into a concrete engine language code, or `null` meaning
 * "no filtering".
 *
 * `follow-ui` is resolved against the *current* UI language rather than being
 * materialised at write time, so changing the display language re-filters the model
 * list instead of leaving a stale choice behind.
 */
export function resolveLanguageFilter(
  filter: LanguageFilter,
  uiLanguage: string,
): string | null {
  if (filter === ALL_LANGUAGES) return null;
  if (filter === FOLLOW_UI_LANGUAGE) return uiLanguageToEngineCode(uiLanguage);
  return filter;
}

/**
 * Resolve the *transcription language* preference into a concrete engine code, or
 * `null` when the choice imposes no constraint on which models are usable.
 *
 * `auto` and `auto-translate` hand the decision to the engine, so every model stays
 * eligible. `follow-ui` tracks the display language. Anything else is a real
 * language the engine must actually support.
 */
export function resolveTranscriptionLanguage(
  language: string | null | undefined,
  uiLanguage: string,
): string | null {
  if (!language || language === 'auto' || language === 'auto-translate') return null;
  if (language === FOLLOW_UI_LANGUAGE) return uiLanguageToEngineCode(uiLanguage);
  // Chinese scripts collapse: engines take `zh`, not `zh-Hans`.
  return language.split('-')[0];
}

/**
 * The value to hand the speech engine, as opposed to the value we filter models by.
 *
 * Same as `resolveTranscriptionLanguage`, except `auto` / `auto-translate` are passed
 * through verbatim — the engines understand them (Whisper translates on
 * `auto-translate`), whereas `follow-ui` is ours alone and must never be sent down.
 */
export function resolveTranscriptionLanguagePreference(
  language: string,
  uiLanguage: string,
): string {
  if (language === FOLLOW_UI_LANGUAGE) return uiLanguageToEngineCode(uiLanguage);
  return language;
}

/** Does this provider support the (already resolved) language? */
export function providerSupportsLanguage(provider: string, language: string | null): boolean {
  if (!language) return true;
  const supported = PROVIDER_LANGUAGES[provider];
  // Unknown providers (cloud ones) are not language-constrained.
  if (!supported) return true;
  return supported.includes(language);
}

/** Does this model support the (already resolved) language? */
export function modelSupportsLanguage(
  modelLanguages: string[] | undefined,
  language: string | null,
): boolean {
  if (!language) return true;
  // A model that doesn't declare languages is treated as unconstrained rather than
  // hidden — better to show a model we can't classify than to silently lose it.
  if (!modelLanguages || modelLanguages.length === 0) return true;
  return modelLanguages.includes(language);
}

/** Display labels for the language badges on model cards. */
export const LANGUAGE_BADGES: Record<string, string> = {
  en: 'EN',
  zh: '中文',
  ja: '日本語',
  ko: '한국어',
  yue: '粵語',
};
