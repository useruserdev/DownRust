// SPDX-License-Identifier: GPL-3.0-or-later
import {useUi} from '../state/ui';

/** Queues a toast message. ToastBridge shows it. */
export function notify(type: 'info' | 'error', body: string): void {
  useUi.getState().pushNotification(type, body);
}

/** Turns any thrown value into readable text. */
export function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  if (typeof err === 'object' && err !== null && 'message' in err && typeof err.message === 'string') {
    return err.message;
  }
  return String(err);
}
