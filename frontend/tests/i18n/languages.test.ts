import { describe, expect, test } from 'bun:test';

import {
  DEFAULT_UI_LANGUAGE,
  SUPPORTED_UI_LANGUAGES,
  resolveUiLanguage,
  uiLanguageToEngineCode,
} from '../../src/i18n/languages';

describe('resolveUiLanguage', () => {
  test('Simplified Chinese locales from macOS and Windows all map to zh-Hans', () => {
    for (const tag of ['zh-Hans', 'zh-Hans-CN', 'zh-CN', 'zh-SG', 'zh', 'ZH-cn']) {
      expect(resolveUiLanguage(tag)).toBe('zh-Hans');
    }
  });

  test('Traditional Chinese falls back to English rather than showing Simplified', () => {
    for (const tag of ['zh-Hant', 'zh-TW', 'zh-HK', 'zh-MO', 'zh-Hant-TW']) {
      expect(resolveUiLanguage(tag)).toBe(DEFAULT_UI_LANGUAGE);
    }
  });

  test('English variants and unsupported languages resolve to English', () => {
    for (const tag of ['en', 'en-US', 'en-GB', 'fr-FR', 'ja', 'de']) {
      expect(resolveUiLanguage(tag)).toBe('en');
    }
  });

  test('missing input resolves to the default', () => {
    expect(resolveUiLanguage(null)).toBe(DEFAULT_UI_LANGUAGE);
    expect(resolveUiLanguage(undefined)).toBe(DEFAULT_UI_LANGUAGE);
    expect(resolveUiLanguage('')).toBe(DEFAULT_UI_LANGUAGE);
  });

  test('whatever it returns is always a language the app ships', () => {
    for (const tag of ['zh-Hans', 'zh-TW', 'xx', 'en-AU', 'pt-BR']) {
      expect(SUPPORTED_UI_LANGUAGES).toContain(resolveUiLanguage(tag));
    }
  });
});

describe('uiLanguageToEngineCode', () => {
  test('strips the script so engines receive a plain ISO-639-1 code', () => {
    expect(uiLanguageToEngineCode('zh-Hans')).toBe('zh');
    expect(uiLanguageToEngineCode('en')).toBe('en');
  });
});

describe('supported languages', () => {
  test('English sorts before Chinese in the picker', () => {
    expect(SUPPORTED_UI_LANGUAGES).toEqual(['en', 'zh-Hans']);
  });
});
