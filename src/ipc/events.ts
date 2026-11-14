// SPDX-License-Identifier: GPL-3.0-or-later
import {listen} from '@tauri-apps/api/event';
import type {UnlistenFn} from '@tauri-apps/api/event';
import {mockListen} from './mock';
import {inTauri} from './runtime';
import {AFTER_QUEUE_CANCELLED_EVENT, AFTER_QUEUE_COUNTDOWN_EVENT} from './types';
import type {
  AfterQueueCountdown,
  AppSettings,
  DownloadId,
  DownloadRecord,
  ExternalAddRequest,
  OpenBatchPayload,
  ProgressBatch,
} from './types';

/** Event names emitted by the backend. */
export const EVENTS = {
  downloadAdded: 'download-added',
  downloadUpdated: 'download-updated',
  downloadCompleted: 'download-completed',
  downloadRemoved: 'download-removed',
  downloadProgress: 'download-progress',
  clipboardUrl: 'clipboard-url',
  openAddDialog: 'open-add-dialog',
  openBatchDialog: 'open-batch-dialog',
  externalAddRequest: 'external-add-request',
  settingsChanged: 'settings-changed',
  afterQueueCountdown: AFTER_QUEUE_COUNTDOWN_EVENT,
  afterQueueCancelled: AFTER_QUEUE_CANCELLED_EVENT,
} as const;

export interface EventPayloads {
  'download-added': DownloadRecord;
  'download-updated': DownloadRecord;
  'download-completed': DownloadRecord;
  'download-removed': {id: DownloadId};
  'download-progress': ProgressBatch;
  'clipboard-url': {url: string};
  'open-add-dialog': {url: string | null};
  'open-batch-dialog': OpenBatchPayload;
  'external-add-request': ExternalAddRequest;
  'settings-changed': AppSettings;
  'after-queue-countdown': AfterQueueCountdown;
  'after-queue-cancelled': undefined;
}

export type EventName = keyof EventPayloads;

/**
 * Subscribes to a backend event. The returned promise resolves to the unsubscribe function.
 * Call the unsubscribe function in the effect cleanup.
 */
export async function onEvent<K extends EventName>(
  name: K,
  handler: (payload: EventPayloads[K]) => void,
): Promise<UnlistenFn> {
  if (inTauri) {
    return listen<EventPayloads[K]>(name, event => handler(event.payload));
  }
  return mockListen(name, payload => handler(payload as EventPayloads[K]));
}
