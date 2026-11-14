// SPDX-License-Identifier: GPL-3.0-or-later
import {useSyncExternalStore} from 'react';
import {en} from './en';
import {ru} from './ru';
import type {MessageKey, Messages, PluralForms} from './types';

export type {MessageKey, Messages, PluralForms} from './types';

/** The language choice stored in the settings. */
export type LanguagePreference = 'system' | 'en' | 'ru';
/** The language the screen really uses. */
export type Lang = 'en' | 'ru';
export type Params = Record<string, string | number>;

const DICTIONARIES: Record<Lang, Messages> = {en, ru};

let preference: LanguagePreference = 'system';
let currentLang: Lang = resolveLang('system');
const listeners = new Set<() => void>();

/** Picks the screen language. 'system' follows the browser or Windows language: Russian when it starts with ru. */
export function resolveLang(pref: LanguagePreference, systemLocale?: string): Lang {
  if (pref === 'en' || pref === 'ru') return pref;
  const locale = systemLocale ?? (typeof navigator === 'undefined' ? 'en' : navigator.language);
  return locale.toLowerCase().startsWith('ru') ? 'ru' : 'en';
}

/** Sets the language choice. Subscribed components re-render when the resolved language changes. */
export function setLanguagePreference(pref: LanguagePreference): void {
  preference = pref;
  const next = resolveLang(pref);
  if (next === currentLang) return;
  currentLang = next;
  for (const listener of listeners) listener();
}

export function getLanguagePreference(): LanguagePreference {
  return preference;
}

export function getLang(): Lang {
  return currentLang;
}

/** The BCP 47 tag used for Intl formatting. */
export function localeTag(lang: Lang = currentLang): string {
  return lang === 'ru' ? 'ru-RU' : 'en-US';
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function lookup(lang: Lang, key: string): unknown {
  let node: unknown = DICTIONARIES[lang];
  for (const part of key.split('.')) {
    if (typeof node !== 'object' || node === null) return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return node;
}

function isPluralForms(value: unknown): value is PluralForms {
  return typeof value === 'object' && value !== null && 'other' in value && 'one' in value;
}

function interpolate(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/**
 * Looks up a message in the current language and fills its placeholders.
 * A plural entry picks its form from `params.count` with Intl.PluralRules.
 * A missing key falls back to English, then to the key itself.
 */
export function t(key: MessageKey, params?: Params): string {
  const value = lookup(currentLang, key) ?? lookup('en', key);
  if (typeof value === 'string') return interpolate(value, params);
  if (isPluralForms(value)) {
    const form = new Intl.PluralRules(localeTag()).select(Number(params?.count ?? 0));
    const text = (form === 'one' || form === 'few' || form === 'many' ? value[form] : undefined) ?? value.other;
    return interpolate(text, params);
  }
  return key;
}

/** Subscribes the calling component to language changes. Returns the translator and the current language. */
export function useT(): {t: typeof t; lang: Lang} {
  const lang = useSyncExternalStore(subscribe, getLang, getLang);
  return {t, lang};
}
