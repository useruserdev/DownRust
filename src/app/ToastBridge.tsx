// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {useToast} from '@astryxdesign/core/Toast';
import {useUi} from '../state/ui';
import {t} from '../i18n';

/** Name shown for a link in the clipboard toast: the last path segment, or the host when there is none. */
function linkName(url: string): string {
  try {
    const parsed = new URL(url);
    const last = parsed.pathname.split('/').filter(Boolean).pop();
    return last ? decodeURIComponent(last) : parsed.host;
  } catch {
    return url;
  }
}

/** Shows each queued notice as a toast, then removes it from the queue. Renders nothing itself. */
export function ToastBridge() {
  const toast = useToast();
  const notifications = useUi(state => state.notifications);
  const dismissNotification = useUi(state => state.dismissNotification);
  const clipboardUrl = useUi(state => state.clipboardUrl);
  const setClipboardUrl = useUi(state => state.setClipboardUrl);
  const openAdd = useUi(state => state.openAdd);

  useEffect(() => {
    for (const notice of notifications) {
      toast({
        type: notice.type,
        body: notice.body,
        uniqueID: `notice-${notice.id}`,
        collisionBehavior: 'ignore',
      });
      dismissNotification(notice.id);
    }
  }, [notifications, dismissNotification, toast]);

  useEffect(() => {
    if (clipboardUrl === null) return;
    const url = clipboardUrl;
    toast({
      body: t('toast.linkCopied', {name: linkName(url)}),
      uniqueID: 'clipboard-link',
      collisionBehavior: 'overwrite',
      endContent: (
        <Button
          label={t('toast.download')}
          variant="ghost"
          size="sm"
          onClick={() => openAdd({url})}
        />
      ),
    });
    setClipboardUrl(null);
  }, [clipboardUrl, openAdd, setClipboardUrl, toast]);

  return null;
}
