// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, VStack} from '@astryxdesign/core/Layout';
import {Text} from '@astryxdesign/core/Text';
import {api} from '../ipc/commands';
import type {AfterQueueAction, AfterQueueCountdown} from '../ipc/types';
import {errorText, notify} from '../lib/notify';
import {useUi} from '../state/ui';
import {t} from '../i18n';
import type {MessageKey} from '../i18n';

type ActiveAction = Exclude<AfterQueueAction, 'none'>;

const ACTION_TITLE: Record<ActiveAction, MessageKey> = {
  exit: 'afterQueue.titleExit',
  sleep: 'afterQueue.titleSleep',
  hibernate: 'afterQueue.titleHibernate',
  shutdown: 'afterQueue.titleShutdown',
};

/** Countdown shown when the queue is empty and an after-queue action is set. Driven by `useUi().countdown`. */
export function AfterQueueDialog() {
  const countdown = useUi(state => state.countdown);
  if (countdown === null || countdown.action === 'none') return null;
  return <Countdown key={`${countdown.action}-${countdown.seconds}`} countdown={countdown} />;
}

function Countdown({countdown}: {countdown: AfterQueueCountdown}) {
  const [remaining, setRemaining] = useState(countdown.seconds);
  const setCountdown = useUi(state => state.setCountdown);
  const setAfterQueue = useUi(state => state.setAfterQueue);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setRemaining(value => Math.max(0, value - 1));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);

  const action = countdown.action as ActiveAction;

  function cancel() {
    api
      .cancelAfterQueue()
      .then(() => {
        setCountdown(null);
        setAfterQueue('none');
      })
      .catch(err => notify('error', errorText(err)));
  }

  return (
    <Dialog isOpen onOpenChange={() => undefined} purpose="required" width={420}>
      <DialogHeader title={t(ACTION_TITLE[action], {seconds: remaining})} />
      <VStack gap={4}>
        <Text type="supporting" color="secondary">
          {t('afterQueue.hint')}
        </Text>
        <HStack gap={2} hAlign="end">
          <Button label={t('common.cancel')} variant="primary" clickAction={cancel} />
        </HStack>
      </VStack>
    </Dialog>
  );
}
