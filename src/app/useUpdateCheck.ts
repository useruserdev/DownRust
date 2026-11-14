// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect} from 'react';
import {onEvent} from '../ipc/events';
import {notify} from '../lib/notify';
import {t} from '../i18n';

/**
 * Shows a notification when the backend reports a new version. The backend sends each version
 * once per run.
 */
export function useUpdateCheck(): void {
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    onEvent('update-available', info => {
      if (!info.available || !info.version) return;
      notify('info', t('update.available', {version: info.version}));
    }).then(stop => {
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);
}
