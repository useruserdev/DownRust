// SPDX-License-Identifier: GPL-3.0-or-later

/** True when the page runs inside the Tauri webview, false in a plain browser. */
export const inTauri: boolean = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
