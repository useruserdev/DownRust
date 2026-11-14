// SPDX-License-Identifier: GPL-3.0-or-later
import {create} from 'zustand';
import {api} from '../ipc/commands';
import type {AppSettings} from '../ipc/types';
import {errorText, notify} from '../lib/notify';
import {setLanguagePreference} from '../i18n';

export type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};

export type SettingsPatch = DeepPartial<AppSettings>;

const SAVE_DELAY_MS = 300;

let saveTimer: ReturnType<typeof setTimeout> | null = null;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function deepMerge(target: unknown, patch: unknown): unknown {
  if (!isPlainObject(target) || !isPlainObject(patch)) return patch;
  const merged: Record<string, unknown> = {...target};
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) continue;
    merged[key] = deepMerge(merged[key], value);
  }
  return merged;
}

export interface SettingsState {
  value: AppSettings | null;
  load: () => Promise<void>;
  update: (patch: SettingsPatch) => void;
  applyRemote: (settings: AppSettings) => void;
}

export const useSettings = create<SettingsState>()((set, get) => ({
  value: null,
  load: async () => {
    try {
      const settings = await api.getSettings();
      setLanguagePreference(settings.language);
      set({value: settings});
    } catch (err) {
      notify('error', errorText(err));
    }
  },
  update: patch => {
    const current = get().value;
    if (!current) return;
    const merged = deepMerge(current, patch) as AppSettings;
    setLanguagePreference(merged.language);
    set({value: merged});
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = null;
      const latest = get().value;
      if (!latest) return;
      api
        .updateSettings(latest)
        .then(saved => {
          if (saveTimer === null) {
            setLanguagePreference(saved.language);
            set({value: saved});
          }
        })
        .catch(err => notify('error', errorText(err)));
    }, SAVE_DELAY_MS);
  },
  applyRemote: settings => {
    // A pending local edit is newer than any echo of an earlier save, so keep it.
    if (saveTimer === null) {
      setLanguagePreference(settings.language);
      set({value: settings});
    }
  },
}));
