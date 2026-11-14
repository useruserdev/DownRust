// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useState} from 'react';
import {Badge} from '@astryxdesign/core/Badge';
import {DropdownMenu} from '@astryxdesign/core/DropdownMenu';
import {HStack, LayoutFooter, StackItem} from '@astryxdesign/core/Layout';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Text} from '@astryxdesign/core/Text';
import {SpeedSparkline} from '../../viz/SpeedSparkline';
import {api} from '../../ipc/commands';
import type {AfterQueueAction} from '../../ipc/types';
import {useDownloads} from '../../state/downloads';
import {useSettings} from '../../state/settings';
import type {SettingsPatch} from '../../state/settings';
import {useSpeedHistory} from '../../state/speedHistory';
import {useStatusCounts} from '../../state/selectors';
import {useUi} from '../../state/ui';
import {humanSpeed} from '../../lib/format';
import {errorText, notify} from '../../lib/notify';
import {isOpenNow} from '../../lib/schedule';
import {t} from '../../i18n';
import type {MessageKey} from '../../i18n';

const KIB = 1024;
const MIB = 1024 * 1024;
const CLOCK_TICK_MS = 30_000;

const LIMIT_OPTIONS: {label: () => string; value: number | null}[] = [
  {label: () => t('common.noLimit'), value: null},
  {label: () => `512 ${t('units.kbps')}`, value: 512 * KIB},
  {label: () => `1 ${t('units.mbps')}`, value: MIB},
  {label: () => `2 ${t('units.mbps')}`, value: 2 * MIB},
  {label: () => `5 ${t('units.mbps')}`, value: 5 * MIB},
];

const AFTER_QUEUE_OPTIONS: {action: AfterQueueAction; labelKey: MessageKey}[] = [
  {action: 'none', labelKey: 'afterQueue.option.none'},
  {action: 'exit', labelKey: 'afterQueue.option.exit'},
  {action: 'sleep', labelKey: 'afterQueue.option.sleep'},
  {action: 'hibernate', labelKey: 'afterQueue.option.hibernate'},
  {action: 'shutdown', labelKey: 'afterQueue.option.shutdown'},
];

function applySettings(patch: SettingsPatch): void {
  try {
    useSettings.getState().update(patch);
  } catch (err) {
    notify('error', errorText(err));
  }
}

function setAfterQueue(action: AfterQueueAction): void {
  api
    .setAfterQueueAction(action)
    .then(() => useUi.getState().setAfterQueue(action))
    .catch(err => notify('error', errorText(err)));
}

/** The current time, refreshed every half minute. Used to judge the schedule window. */
function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), CLOCK_TICK_MS);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

/** Status footer: total speed with its history, the schedule state, the global speed limit, the parallel count and the after-queue action. */
export function StatusFooter() {
  const totalSpeed = useDownloads(state => state.totalSpeed);
  const samples = useSpeedHistory(state => state.total);
  const limit = useSettings(state => state.value?.engine.globalSpeedLimit ?? null);
  const maxConcurrent = useSettings(state => state.value?.engine.maxConcurrentDownloads ?? 3);
  const schedule = useSettings(state => state.value?.engine.schedule ?? null);
  const queued = useStatusCounts().queued;
  const afterQueue = useUi(state => state.afterQueue);
  const now = useClock();

  const limitLabel = limit === null ? t('common.noLimit') : humanSpeed(limit);
  const waitingForSchedule = schedule !== null && schedule.enabled && queued > 0 && !isOpenNow(schedule, now);
  const afterQueueOption = AFTER_QUEUE_OPTIONS.find(option => option.action === afterQueue) ?? AFTER_QUEUE_OPTIONS[0];
  const afterQueueLabel = t(afterQueueOption.labelKey);

  return (
    <LayoutFooter>
      <HStack gap={3} vAlign="center">
        <Text weight="medium" hasTabularNumbers textWrap="nowrap">
          {humanSpeed(totalSpeed)}
        </Text>
        <SpeedSparkline samples={samples} size="sm" width="compact" />
        {waitingForSchedule && <Badge variant="info" label={t('footer.waitingSchedule')} />}
        <StackItem size="fill" />
        <DropdownMenu
          button={{label: t('footer.whenDone', {action: afterQueueLabel}), variant: 'ghost', size: 'sm'}}
          items={AFTER_QUEUE_OPTIONS.map(option => ({
            label: t(option.labelKey),
            onClick: () => setAfterQueue(option.action),
          }))}
        />
        <DropdownMenu
          button={{label: t('footer.limit', {limit: limitLabel}), variant: 'ghost', size: 'sm'}}
          items={[
            ...LIMIT_OPTIONS.map(option => ({
              label: option.label(),
              onClick: () => applySettings({engine: {globalSpeedLimit: option.value}}),
            })),
            {label: t('menu.custom'), onClick: () => useUi.getState().openSpeedLimit(null)},
          ]}
        />
        <HStack gap={2} vAlign="center">
          <Text type="supporting" color="secondary" textWrap="nowrap">
            {t('footer.atOnce')}
          </Text>
          <NumberInput
            label={t('footer.atOnceLabel')}
            isLabelHidden
            size="sm"
            min={1}
            max={10}
            hasNumberSteppers
            value={maxConcurrent}
            onChange={value => applySettings({engine: {maxConcurrentDownloads: value}})}
          />
        </HStack>
      </HStack>
    </LayoutFooter>
  );
}
