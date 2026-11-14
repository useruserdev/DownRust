// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {Dialog, DialogHeader} from '@astryxdesign/core/Dialog';
import {HStack, VStack} from '@astryxdesign/core/Layout';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {SegmentedControl, SegmentedControlItem} from '@astryxdesign/core/SegmentedControl';
import {Text} from '@astryxdesign/core/Text';
import {api} from '../ipc/commands';
import type {DownloadRecord} from '../ipc/types';
import {humanSpeed} from '../lib/format';
import {errorText, notify} from '../lib/notify';
import {useDownloads} from '../state/downloads';
import {useSettings} from '../state/settings';
import {useUi} from '../state/ui';
import {t} from '../i18n';

const KIB = 1024;
const MIB = 1024 * 1024;

type SpeedUnit = 'KB' | 'MB';
const UNIT_BYTES: Record<SpeedUnit, number> = {KB: KIB, MB: MIB};

/** Picks MB/s when the limit is a whole number of MB/s, otherwise KB/s. */
function unitFor(bytes: number | null): SpeedUnit {
  return bytes !== null && bytes >= MIB && bytes % MIB === 0 ? 'MB' : 'KB';
}

/** Turns a stored byte rate into the number shown for the given unit. */
function displayValue(bytes: number | null, unit: SpeedUnit): number {
  if (bytes === null) return unit === 'MB' ? 1 : 512;
  return Math.max(1, Math.round(bytes / UNIT_BYTES[unit]));
}

/** Sets a speed limit in bytes per second. With no record it sets the global limit. */
function applyLimit(id: DownloadRecord['id'] | null, bytes: number | null): void {
  if (id === null) {
    useSettings.getState().update({engine: {globalSpeedLimit: bytes}});
    useUi.getState().closeSpeedLimit();
    return;
  }
  api
    .setDownloadSpeedLimit(id, bytes)
    .then(() => useUi.getState().closeSpeedLimit())
    .catch(err => notify('error', errorText(err)));
}

/** Speed limit dialog. Sets the global limit, or the limit of one download when opened from its row. */
export function SpeedLimitDialog() {
  const {open, id} = useUi(state => state.speedDialog);
  const closeSpeedLimit = useUi(state => state.closeSpeedLimit);
  const globalLimit = useSettings(state => state.value?.engine.globalSpeedLimit ?? null);
  const record = useDownloads(state => (id === null ? undefined : state.records[id]));

  const current = id === null ? globalLimit : (record?.speedLimit ?? null);
  const target = id === null ? t('speed.targetAll') : (record?.fileName ?? t('speed.targetThis'));

  return (
    <Dialog isOpen={open} onOpenChange={isOpen => !isOpen && closeSpeedLimit()} width={440} purpose="form" padding={4}>
      <DialogHeader title={t('speed.title')} onOpenChange={() => closeSpeedLimit()} />
      <SpeedLimitForm current={current} target={target} id={id} />
    </Dialog>
  );
}

interface SpeedLimitFormProps {
  current: number | null;
  target: string;
  id: DownloadRecord['id'] | null;
}

function SpeedLimitForm({current, target, id}: SpeedLimitFormProps) {
  const [unit, setUnit] = useState<SpeedUnit>(() => unitFor(current));
  const [value, setValue] = useState<number>(() => displayValue(current, unitFor(current)));

  const isValid = Number.isFinite(value) && value >= 1;
  const unitLabel = (next: SpeedUnit) => (next === 'MB' ? t('units.mbps') : t('units.kbps'));

  function changeUnit(next: SpeedUnit) {
    setValue(Math.max(1, Math.round((value * UNIT_BYTES[unit]) / UNIT_BYTES[next])));
    setUnit(next);
  }

  return (
    <VStack gap={4}>
      <Text type="supporting" color="secondary">
        {t('speed.hint', {target, current: current === null ? t('common.noLimitLower') : humanSpeed(current)})}
      </Text>
      <HStack gap={2} vAlign="end">
        <NumberInput
          label={t('speed.title')}
          value={value}
          onChange={setValue}
          min={1}
          isIntegerOnly
          units={unitLabel(unit)}
        />
        <SegmentedControl label={t('speed.unit')} value={unit} onChange={next => changeUnit(next as SpeedUnit)} size="md">
          <SegmentedControlItem value="KB" label={t('units.kbps')} />
          <SegmentedControlItem value="MB" label={t('units.mbps')} />
        </SegmentedControl>
      </HStack>
      <HStack gap={2} hAlign="end">
        <Button label={t('common.noLimit')} variant="secondary" clickAction={() => applyLimit(id, null)} />
        <Button
          label={t('common.apply')}
          variant="primary"
          isDisabled={!isValid}
          clickAction={() => applyLimit(id, Math.round(value * UNIT_BYTES[unit]))}
        />
      </HStack>
    </VStack>
  );
}
