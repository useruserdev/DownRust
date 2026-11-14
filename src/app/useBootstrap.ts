// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect} from 'react';
import type {UnlistenFn} from '@tauri-apps/api/event';
import {api} from '../ipc/commands';
import {onEvent} from '../ipc/events';
import {errorText, notify} from '../lib/notify';
import {useDownloads} from '../state/downloads';
import {useSettings} from '../state/settings';
import {useSpeedHistory} from '../state/speedHistory';
import {useUi} from '../state/ui';

/** Subscribes to backend events, loads the first state and tells the backend the page is ready. */
export function useBootstrap(): void {
  useEffect(() => {
    const subscriptions: Promise<UnlistenFn>[] = [
      onEvent('download-added', record => useDownloads.getState().upsert(record)),
      onEvent('download-updated', record => useDownloads.getState().upsert(record)),
      onEvent('download-completed', record => useDownloads.getState().upsert(record)),
      onEvent('download-removed', ({id}) => useDownloads.getState().remove(id)),
      onEvent('download-progress', batch => {
        useDownloads.getState().applyProgress(batch);
        useSpeedHistory.getState().push(batch);
      }),
      onEvent('clipboard-url', ({url}) => useUi.getState().setClipboardUrl(url)),
      onEvent('open-add-dialog', ({url}) => useUi.getState().openAdd({url: url ?? ''})),
      onEvent('open-batch-dialog', ({text}) => useUi.getState().openBatch(text)),
      onEvent('external-add-request', request => useUi.getState().openAdd(request)),
      onEvent('settings-changed', settings => useSettings.getState().applyRemote(settings)),
      onEvent('after-queue-countdown', countdown => useUi.getState().setCountdown(countdown)),
      onEvent('after-queue-cancelled', () => {
        useUi.getState().setCountdown(null);
        useUi.getState().setAfterQueue('none');
      }),
    ];

    return () => {
      for (const subscription of subscriptions) {
        void subscription.then(unlisten => unlisten());
      }
    };
  }, []);

  useEffect(() => {
    void useSettings.getState().load();
    api
      .listDownloads()
      .then(records => useDownloads.getState().setAll(records))
      .catch(err => notify('error', errorText(err)));
    api
      .getAfterQueueAction()
      .then(action => useUi.getState().setAfterQueue(action))
      .catch(err => notify('error', errorText(err)));
    api.frontendReady().catch(err => notify('error', errorText(err)));
  }, []);
}
