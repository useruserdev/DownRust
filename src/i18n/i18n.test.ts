// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it, beforeEach} from 'vitest';
import {en} from './en';
import {ru} from './ru';
import {resolveLang, setLanguagePreference, t} from './index';
import {formatEta, humanBytes, humanSpeed} from '../lib/format';

type Node = Record<string, unknown>;

/** Lists every leaf path of a dictionary together with its value. */
function leaves(node: Node, prefix = ''): [string, unknown][] {
  return Object.entries(node).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') return [[path, value] as [string, unknown]];
    if (typeof value === 'object' && value !== null && 'one' in value && 'other' in value) {
      return [[path, value] as [string, unknown]];
    }
    return leaves(value as Node, path);
  });
}

describe('dictionaries', () => {
  it('every English key exists in Russian and is non-empty', () => {
    const russian = new Map(leaves(ru as unknown as Node));
    for (const [path, value] of leaves(en as unknown as Node)) {
      expect(russian.has(path), `missing ru key ${path}`).toBe(true);
      const translated = russian.get(path);
      if (typeof value === 'string') {
        expect(typeof translated === 'string' && translated.trim() !== '', `empty ru key ${path}`).toBe(true);
      } else {
        const forms = translated as Record<string, string>;
        for (const form of ['one', 'few', 'many', 'other']) {
          expect(typeof forms[form] === 'string' && forms[form].trim() !== '', `ru ${path}.${form}`).toBe(true);
        }
      }
    }
  });

  it('Russian has no key that English lacks', () => {
    const english = new Set(leaves(en as unknown as Node).map(([path]) => path));
    for (const [path] of leaves(ru as unknown as Node)) {
      expect(english.has(path), `extra ru key ${path}`).toBe(true);
    }
  });

  it('no Russian text uses the forbidden wording', () => {
    for (const [path, value] of leaves(ru as unknown as Node)) {
      const text = typeof value === 'string' ? value : Object.values(value as Record<string, string>).join(' ');
      const forbidden =
        /(^|\W)ИИ(\W|$)/.test(text) ||
        /нейросет|сгенерир/i.test(text);
      expect(forbidden, `forbidden wording in ${path}`).toBe(false);
    }
  });
});

describe('t()', () => {
  beforeEach(() => setLanguagePreference('en'));

  it('reads English and Russian', () => {
    expect(t('common.cancel')).toBe('Cancel');
    setLanguagePreference('ru');
    expect(t('common.cancel')).toBe('Отмена');
  });

  it('interpolates named parameters', () => {
    expect(t('transfer.ofTotal', {done: '1 MB', total: '4 MB'})).toBe('1 MB of 4 MB');
    setLanguagePreference('ru');
    expect(t('transfer.ofTotal', {done: '1 МБ', total: '4 МБ'})).toBe('1 МБ из 4 МБ');
  });

  it('picks English plural forms', () => {
    expect(t('stats.files', {count: 1})).toBe('1 file');
    expect(t('stats.files', {count: 5})).toBe('5 files');
  });

  it('picks Russian plural forms', () => {
    setLanguagePreference('ru');
    expect(t('remove.titleMany', {count: 1})).toBe('Удалить 1 загрузку?');
    expect(t('remove.titleMany', {count: 2})).toBe('Удалить 2 загрузки?');
    expect(t('remove.titleMany', {count: 5})).toBe('Удалить 5 загрузок?');
    expect(t('remove.titleMany', {count: 21})).toBe('Удалить 21 загрузку?');
    expect(t('stats.files', {count: 11})).toBe('11 файлов');
  });

  it('resolves the system language from the locale', () => {
    expect(resolveLang('system', 'ru-RU')).toBe('ru');
    expect(resolveLang('system', 'en-US')).toBe('en');
    expect(resolveLang('en', 'ru-RU')).toBe('en');
    expect(resolveLang('ru', 'en-US')).toBe('ru');
  });
});

describe('unit formatting', () => {
  it('formats bytes and speed in English', () => {
    setLanguagePreference('en');
    expect(humanBytes(512)).toBe('512 B');
    expect(humanBytes(1536)).toBe('1.50 KB');
    expect(humanSpeed(2 * 1024 * 1024)).toBe('2.00 MB/s');
  });

  it('formats bytes and speed in Russian with a decimal comma', () => {
    setLanguagePreference('ru');
    expect(humanBytes(1536)).toBe('1,50 КБ');
    expect(humanSpeed(2 * 1024 * 1024)).toBe('2,00 МБ/с');
  });

  it('formats remaining time in both languages', () => {
    setLanguagePreference('en');
    expect(formatEta(42)).toBe('42 s');
    expect(formatEta(720)).toBe('12 min');
    expect(formatEta(3 * 3600 + 7 * 60)).toBe('3 h 07 min');
    expect(formatEta(2 * 86400 + 4 * 3600)).toBe('2 d 4 h');
    setLanguagePreference('ru');
    expect(formatEta(42)).toBe('42 с');
    expect(formatEta(3 * 3600 + 7 * 60)).toBe('3 ч 07 мин');
    expect(formatEta(2 * 86400 + 4 * 3600)).toBe('2 д 4 ч');
  });
});
