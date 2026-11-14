// SPDX-License-Identifier: GPL-3.0-or-later
import type {QueueMove} from '../../ipc/types';
import type {MessageKey} from '../../i18n';

/** Queue move choices in display order, with the keys of their labels. Shared by the row menu and the details panel. */
export const QUEUE_MOVES: {to: QueueMove; labelKey: MessageKey}[] = [
  {to: 'top', labelKey: 'queue.top'},
  {to: 'up', labelKey: 'queue.up'},
  {to: 'down', labelKey: 'queue.down'},
  {to: 'bottom', labelKey: 'queue.bottom'},
];

/** Statuses from which the address of a download can be changed. */
export function canUpdateAddress(status: string): boolean {
  return status === 'paused' || status === 'error' || status === 'queued';
}
