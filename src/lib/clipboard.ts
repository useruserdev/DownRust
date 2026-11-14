// SPDX-License-Identifier: GPL-3.0-or-later
import {writeText} from '@tauri-apps/plugin-clipboard-manager';
import {inTauri} from '../ipc/runtime';

/** Writes text to the system clipboard through Tauri in the app and through the browser API otherwise. */
export async function copyText(text: string): Promise<void> {
  if (inTauri) {
    await writeText(text);
    return;
  }
  await navigator.clipboard.writeText(text);
}
