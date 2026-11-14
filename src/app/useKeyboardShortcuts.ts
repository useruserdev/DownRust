// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect} from 'react';
import {readText} from '@tauri-apps/plugin-clipboard-manager';
import {api} from '../ipc/commands';
import {inTauri} from '../ipc/runtime';
import {errorText, notify} from '../lib/notify';
import {isEditableTarget} from '../lib/keyboard';
import {useDownloads} from '../state/downloads';
import {selectVisibleRows} from '../state/selectors';
import {useSettings} from '../state/settings';
import {useUi} from '../state/ui';
import type {DownloadRecord} from '../ipc/types';
import {t} from '../i18n';

const CONTROL_SELECTOR = 'button, a, [role="button"], [role="menuitem"], [role="checkbox"], [role="option"]';

/** Returns the text when it is an http or https link, otherwise null. */
export function asHttpUrl(text: string | null): string | null {
  if (text === null) return null;
  const trimmed = text.trim();
  try {
    const url = new URL(trimmed);
    return url.protocol === 'http:' || url.protocol === 'https:' ? trimmed : null;
  } catch {
    return null;
  }
}

/** Reads the system clipboard as text. Returns null when the read is refused or fails. */
export async function readClipboardText(): Promise<string | null> {
  try {
    if (inTauri) return await readText();
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
}

function focusSearch(): void {
  const input = document.querySelector<HTMLInputElement>(`input[aria-label="${t('downloads.search')}"]`);
  input?.focus();
  input?.select();
}

function moveSelection(rows: DownloadRecord[], step: number): void {
  if (rows.length === 0) return;
  const {selectedId, select} = useDownloads.getState();
  const index = rows.findIndex(row => row.id === selectedId);
  const next = index < 0 ? (step > 0 ? 0 : rows.length - 1) : Math.min(rows.length - 1, Math.max(0, index + step));
  select(rows[next].id);
}

async function toggleSelected(record: DownloadRecord): Promise<void> {
  try {
    if (record.status === 'downloading' || record.status === 'queued') {
      await api.pauseDownload(record.id);
    } else if (record.status === 'paused' || record.status === 'error') {
      await api.resumeDownload(record.id);
    }
  } catch (err) {
    notify('error', errorText(err));
  }
}

async function openSelected(record: DownloadRecord): Promise<void> {
  if (record.status !== 'completed') return;
  try {
    await api.openFile(record.id);
  } catch (err) {
    notify('error', errorText(err));
  }
}

/** Global keyboard shortcuts for the main window. Text fields and open dialogs keep their own keys. */
export function useKeyboardShortcuts(): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const ui = useUi.getState();
      const dialogOpen =
        ui.addDialog.open || ui.batchDialog.open || ui.removeDialog.open || ui.speedDialog.open;

      if (event.key === 'Escape') {
        if (dialogOpen) return;
        if (ui.view === 'settings' || ui.view === 'stats') ui.setView('downloads');
        return;
      }
      if (dialogOpen || isEditableTarget(event.target)) return;

      const mod = event.ctrlKey || event.metaKey;
      if (mod && event.shiftKey && !event.altKey && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        ui.openBatch();
        return;
      }
      if (mod && !event.altKey && !event.shiftKey) {
        const key = event.key.toLowerCase();
        if (key === 'n') {
          event.preventDefault();
          ui.openAdd();
        } else if (key === 'f') {
          event.preventDefault();
          if (ui.view !== 'downloads') ui.setView('downloads');
          window.setTimeout(focusSearch, 0);
        } else if (key === ',') {
          event.preventDefault();
          ui.setView('settings');
        } else if (key === 'v') {
          void readClipboardText().then(text => {
            const url = asHttpUrl(text);
            if (url !== null) useUi.getState().openAdd({url});
          });
        }
        return;
      }
      if (mod || event.altKey) return;

      const target = event.target;
      const onControl = target instanceof HTMLElement && target.closest(CONTROL_SELECTOR) !== null;
      const downloads = useDownloads.getState();
      const record = downloads.selectedId === null ? undefined : downloads.records[downloads.selectedId];

      switch (event.key) {
        case 'Delete':
          if (record) {
            event.preventDefault();
            ui.openRemove(record.id);
          }
          break;
        case ' ':
          if (record && !onControl) {
            event.preventDefault();
            void toggleSelected(record);
          }
          break;
        case 'Enter':
          if (record && !onControl) {
            event.preventDefault();
            void openSelected(record);
          }
          break;
        case 'ArrowUp':
        case 'ArrowDown':
          if (ui.view === 'downloads') {
            event.preventDefault();
            moveSelection(selectVisibleRows(downloads), event.key === 'ArrowDown' ? 1 : -1);
          }
          break;
        default:
          break;
      }
    };

    const onFocus = () => {
      if (!useSettings.getState().value?.clipboardMonitor) return;
      void readClipboardText().then(text => {
        const url = asHttpUrl(text);
        if (url !== null && url !== useUi.getState().clipboardUrl) {
          useUi.getState().setClipboardUrl(url);
        }
      });
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('focus', onFocus);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('focus', onFocus);
    };
  }, []);
}
