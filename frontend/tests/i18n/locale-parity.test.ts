import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const LOCALES = join(import.meta.dir, '../../src/i18n/locales');

type Json = { [key: string]: Json | string };

function flatten(value: Json | string, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value).reduce<Record<string, string>>(
    (acc, [key, child]) => ({ ...acc, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}

const load = (locale: string, file: string) =>
  flatten(JSON.parse(readFileSync(join(LOCALES, locale, file), 'utf8')));

const files = readdirSync(join(LOCALES, 'en')).filter((f) => f.endsWith('.json'));
const placeholders = (text: string) => [...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map((m) => m[1]).sort();

describe('English and Simplified Chinese stay in sync', () => {
  test('both locales have the same set of files', () => {
    expect(readdirSync(join(LOCALES, 'zh-Hans')).filter((f) => f.endsWith('.json')).sort()).toEqual([...files].sort());
  });

  for (const file of files) {
    describe(file, () => {
      const en = load('en', file);
      const zh = load('zh-Hans', file);

      test('no key is missing from either language', () => {
        const missingInZh = Object.keys(en).filter((k) => !(k in zh));
        const missingInEn = Object.keys(zh).filter((k) => !(k in en));
        expect({ missingInZh, missingInEn }).toEqual({ missingInZh: [], missingInEn: [] });
      });

      test('no translation is empty', () => {
        const empty = Object.entries(zh).filter(([, v]) => v.trim() === '').map(([k]) => k);
        expect(empty).toEqual([]);
      });

      test('interpolation placeholders match between languages', () => {
        const mismatched = Object.keys(en)
          .filter((k) => k in zh)
          .filter((k) => placeholders(en[k]).join() !== placeholders(zh[k]).join());
        expect(mismatched).toEqual([]);
      });
    });
  }
});
