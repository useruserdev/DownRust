// SPDX-License-Identifier: GPL-3.0-or-later
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {Switch} from '@astryxdesign/core/Switch';
import {TimeInput} from '@astryxdesign/core/TimeInput';
import type {ISOTimeString} from '@astryxdesign/core/TimeInput';
import {ToggleButton, ToggleButtonGroup} from '@astryxdesign/core/ToggleButton';
import type {AppSettings, Schedule} from '../../ipc/types';
import type {SettingsPatch} from '../../state/settings';
import {minuteToTime, scheduleSummary, timeToMinute, WEEKDAY_NAMES, dayLabel} from '../../lib/schedule';
import {t} from '../../i18n';
import {SpeedScheduleCard} from './SpeedScheduleCard';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

/** Schedule settings: a time window and the days on which downloads may run. */
export function ScheduleSection({settings, update}: SectionProps) {
  const schedule: Schedule = settings.engine.schedule;
  const isOn = schedule.enabled;

  const setStart = (value: ISOTimeString | undefined) => {
    const minute = timeToMinute(value ?? '');
    if (minute !== null) update({engine: {schedule: {startMinute: minute}}});
  };
  const setStop = (value: ISOTimeString | undefined) => {
    const minute = timeToMinute(value ?? '');
    if (minute !== null) update({engine: {schedule: {stopMinute: minute}}});
  };
  const setDays = (selected: string[]) => {
    const days = WEEKDAY_NAMES.map(name => selected.includes(name)) as Schedule['days'];
    update({engine: {schedule: {days}}});
  };
  const selectedDays = WEEKDAY_NAMES.filter((_, index) => schedule.days[index]);

  return (
    <VStack gap={5}>
      <Heading level={2}>{t('settings.section.schedule')}</Heading>

      <Switch
        label={t('settings.schedule.use')}
        description={t('settings.schedule.useHint')}
        value={isOn}
        onChange={value => update({engine: {schedule: {enabled: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      <HStack gap={3} vAlign="end">
        <StackItem size="fill">
          <TimeInput
            label={t('settings.schedule.start')}
            value={minuteToTime(schedule.startMinute) as ISOTimeString}
            onChange={setStart}
            isDisabled={!isOn}
            hasSeconds={false}
          />
        </StackItem>
        <StackItem size="fill">
          <TimeInput
            label={t('settings.schedule.stop')}
            value={minuteToTime(schedule.stopMinute) as ISOTimeString}
            onChange={setStop}
            isDisabled={!isOn}
            hasSeconds={false}
          />
        </StackItem>
      </HStack>

      <ToggleButtonGroup
        label={t('settings.schedule.days')}
        type="multiple"
        value={selectedDays}
        onChange={value => setDays(Array.isArray(value) ? value : [])}
        isDisabled={!isOn}>
        {WEEKDAY_NAMES.map(name => (
          <ToggleButton key={name} value={name} label={dayLabel(name)}>
            {dayLabel(name)}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      <Text type="supporting" color="secondary">
        {scheduleSummary(schedule)}
      </Text>

      <SpeedScheduleCard settings={settings} update={update} />
    </VStack>
  );
}
