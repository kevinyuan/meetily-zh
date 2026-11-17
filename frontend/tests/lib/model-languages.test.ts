import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  ALL_LANGUAGES,
  PROVIDER_LANGUAGES,
  modelSupportsLanguage,
  providerSupportsLanguage,
  resolveLanguageFilter,
  resolveTranscriptionLanguage,
  resolveTranscriptionLanguagePreference,
} from '../../src/lib/model-languages';
import { getProviderCommands } from '../../src/lib/transcription-model-readiness';
import { FOLLOW_UI_LANGUAGE } from '../../src/i18n/languages';
import { SENSEVOICE_MODEL_CONFIGS } from '../../src/lib/sensevoice';

const rust = (file: string) => readFileSync(join(import.meta.dir, '../../src-tauri/src', file), 'utf8');

describe('language filter resolution', () => {
  test('"all" means no filtering', () => {
    expect(resolveLanguageFilter(ALL_LANGUAGES, 'zh-Hans')).toBeNull();
  });

  test('follow-ui tracks the current display language instead of freezing it', () => {
    expect(resolveLanguageFilter(FOLLOW_UI_LANGUAGE, 'zh-Hans')).toBe('zh');
    expect(resolveLanguageFilter(FOLLOW_UI_LANGUAGE, 'en')).toBe('en');
  });

  test('an explicit language is used as stored', () => {
    expect(resolveLanguageFilter('ja', 'zh-Hans')).toBe('ja');
  });
});

describe('transcription language resolution', () => {
  test('auto, auto-translate and empty values impose no constraint', () => {
    for (const value of ['auto', 'auto-translate', '', null, undefined]) {
      expect(resolveTranscriptionLanguage(value, 'zh-Hans')).toBeNull();
    }
  });

  test('follow-ui resolves against the display language', () => {
    expect(resolveTranscriptionLanguage(FOLLOW_UI_LANGUAGE, 'zh-Hans')).toBe('zh');
  });

  test('script-qualified Chinese collapses to the plain engine code', () => {
    expect(resolveTranscriptionLanguage('zh-Hans', 'en')).toBe('zh');
    expect(resolveTranscriptionLanguage('zh-CN', 'en')).toBe('zh');
  });

  test('the value sent to the engine passes auto through but never leaks follow-ui', () => {
    expect(resolveTranscriptionLanguagePreference('auto', 'zh-Hans')).toBe('auto');
    expect(resolveTranscriptionLanguagePreference('auto-translate', 'zh-Hans')).toBe('auto-translate');
    expect(resolveTranscriptionLanguagePreference(FOLLOW_UI_LANGUAGE, 'zh-Hans')).toBe('zh');
    expect(resolveTranscriptionLanguagePreference(FOLLOW_UI_LANGUAGE, 'zh-Hans')).not.toBe(FOLLOW_UI_LANGUAGE);
  });
});

describe('provider and model language support', () => {
  test('Parakeet is never offered for Chinese', () => {
    expect(providerSupportsLanguage('parakeet', 'zh')).toBeFalse();
    expect(providerSupportsLanguage('parakeet', 'en')).toBeTrue();
  });

  test('SenseVoice and Whisper are offered for Chinese', () => {
    expect(providerSupportsLanguage('senseVoice', 'zh')).toBeTrue();
    expect(providerSupportsLanguage('localWhisper', 'zh')).toBeTrue();
  });

  test('SenseVoice covers Cantonese, Japanese and Korean; Whisper (as tagged) does not', () => {
    for (const language of ['yue', 'ja', 'ko']) {
      expect(providerSupportsLanguage('senseVoice', language)).toBeTrue();
      expect(providerSupportsLanguage('localWhisper', language)).toBeFalse();
    }
  });

  test('no language, or an unknown (cloud) provider, never filters anything out', () => {
    expect(providerSupportsLanguage('parakeet', null)).toBeTrue();
    expect(providerSupportsLanguage('deepgram', 'zh')).toBeTrue();
  });

  test('a model that declares no languages is shown rather than silently hidden', () => {
    expect(modelSupportsLanguage(undefined, 'zh')).toBeTrue();
    expect(modelSupportsLanguage([], 'zh')).toBeTrue();
    expect(modelSupportsLanguage(['en'], 'zh')).toBeFalse();
    expect(modelSupportsLanguage(['en', 'zh'], 'zh')).toBeTrue();
  });
});

describe('frontend/backend consistency', () => {
  // This is the guard that would have caught "SenseVoice: transcription model not ready":
  // a provider the UI offers but the readiness check does not know about.
  test('every local provider the UI offers has readiness commands', () => {
    for (const provider of Object.keys(PROVIDER_LANGUAGES)) {
      expect(getProviderCommands(provider)).not.toBeNull();
    }
  });

  test('the provider list in TranscriptSettings matches PROVIDER_LANGUAGES', () => {
    const source = readFileSync(join(import.meta.dir, '../../src/components/TranscriptSettings.tsx'), 'utf8');
    const declared = /const LOCAL_PROVIDERS = \[([^\]]+)\]/.exec(source)?.[1] ?? '';
    const providers = [...declared.matchAll(/'([^']+)'/g)].map((m) => m[1]).sort();
    expect(providers).toEqual(Object.keys(PROVIDER_LANGUAGES).sort());
  });

  test('Parakeet languages match the Rust catalog, and Rust never lists zh for it', () => {
    const line = /PARAKEET_LANGUAGES: &\[&str\] = &\[([^\]]*)\]/.exec(rust('config.rs'))?.[1] ?? '';
    const languages = [...line.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect(languages).toEqual(PROVIDER_LANGUAGES.parakeet);
    expect(languages).not.toContain('zh');
  });

  test('Whisper languages match the Rust catalog', () => {
    const line = /WHISPER_LANGUAGES: &\[&str\] = &\[([^\]]*)\]/.exec(rust('config.rs'))?.[1] ?? '';
    expect([...line.matchAll(/"([^"]+)"/g)].map((m) => m[1])).toEqual(PROVIDER_LANGUAGES.localWhisper);
  });

  test('SenseVoice languages and download size match the Rust engine', () => {
    const engine = rust('sensevoice_engine/sensevoice_engine.rs');
    const line = /SENSEVOICE_LANGUAGES: &\[&str\] = &\[([^\]]*)\]/.exec(engine)?.[1] ?? '';
    const languages = [...line.matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect([...languages].sort()).toEqual([...PROVIDER_LANGUAGES.senseVoice].sort());
    expect([...(SENSEVOICE_MODEL_CONFIGS['sense-voice-small-int8'].languages ?? [])].sort()).toEqual([...languages].sort());
    expect(engine).toContain(`size_mb: ${SENSEVOICE_MODEL_CONFIGS['sense-voice-small-int8'].size_mb}`);
  });
});
