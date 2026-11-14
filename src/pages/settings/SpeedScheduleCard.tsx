// SPDX-License-Identifier: GPL-3.0-or-later
import {useState} from 'react';
import {Button} from '@astryxdesign/core/Button';
import {HStack, StackItem, VStack} from '@astryxdesign/core/Layout';
import {Heading, Text} from '@astryxdesign/core/Text';
import {IconButton} from '@astryxdesign/core/IconButton';
import {Icon} from '@astryxdesign/core/Icon';
import {NumberInput} from '@astryxdesign/core/NumberInput';
import {Switch} from '@astryxdesign/core/Switch';
import {TimeInput} from '@astryxdesign/core/TimeInput';
import type {ISOTimeString} from '@astryxdesign/core/TimeInput';
import {ToggleButton, ToggleButtonGroup} from '@astryxdesign/core/ToggleButton';
import {Plus, Trash2} from 'lucide-react';
import type {AppSettings, SpeedRule} from '../../ipc/types';
import type {SettingsPatch} from '../../state/settings';
import {dayLabel, minuteToTime, timeToMinute, WEEKDAY_NAMES} from '../../lib/schedule';
import {t} from '../../i18n';

interface SectionProps {
  settings: AppSettings;
  update: (patch: SettingsPatch) => void;
}

const KIBIBYTE = 1024;
const MEBIBYTE = 1024 * 1024;
const ALL_DAYS = [0, 1, 2, 3, 4, 5, 6];

interface RuleRowProps {
  rule: SpeedRule;
  isDisabled: boolean;
  onChange: (rule: SpeedRule) => void;
  onRemove: () => void;
}

/** One speed rule: days, time window and limit. */
function RuleRow({rule, isDisabled, onChange, onRemove}: RuleRowProps) {
  const [unit, setUnit] = useState<'kb' | 'mb'>(rule.limit !== null && rule.limit >= MEBIBYTE ? 'mb' : 'kb');
  const divisor = unit === 'mb' ? MEBIBYTE : KIBIBYTE;

  const setTime = (key: 'start' | 'end', value: ISOTimeString | undefined) => {
    const minute = timeToMinute(value ?? '');
    if (minute !== null) onChange({...rule, [key]: minuteToTime(minute)});
  };
  const selectedDays = WEEKDAY_NAMES.filter((_, index) => rule.days.includes(index));
  const setDays = (selected: string[]) => {
    const days = WEEKDAY_NAMES.flatMap((name, index) => (selected.includes(name) ? [index] : []));
    onChange({...rule, days});
  };

  return (
    <VStack gap={3}>
      <HStack gap={3} vAlign="end">
        <StackItem size="fill">
          <TimeInput
            label={t('speedSchedule.start')}
            value={minuteToTime(timeToMinute(rule.start) ?? 0) as ISOTimeString}
            onChange={value => setTime('start', value)}
            isDisabled={isDisabled}
            hasSeconds={false}
          />
        </StackItem>
        <StackItem size="fill">
          <TimeInput
            label={t('speedSchedule.end')}
            value={minuteToTime(timeToMinute(rule.end) ?? 0) as ISOTimeString}
            onChange={value => setTime('end', value)}
            isDisabled={isDisabled}
            hasSeconds={false}
          />
        </StackItem>
        <IconButton
          label={t('speedSchedule.removeRule')}
          icon={<Icon icon={Trash2} />}
          tooltip={t('speedSchedule.removeRule')}
          onClick={onRemove}
          isDisabled={isDisabled}
        />
      </HStack>

      <ToggleButtonGroup
        label={t('speedSchedule.days')}
        type="multiple"
        value={selectedDays}
        onChange={value => setDays(Array.isArray(value) ? value : [])}
        isDisabled={isDisabled}>
        {WEEKDAY_NAMES.map(name => (
          <ToggleButton key={name} value={name} label={dayLabel(name)}>
            {dayLabel(name)}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      <Switch
        label={t('speedSchedule.noLimit')}
        value={rule.limit === null}
        onChange={value => onChange({...rule, limit: value ? null : MEBIBYTE})}
        isDisabled={isDisabled}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />
      {rule.limit !== null && (
        <HStack gap={3} vAlign="end">
          <NumberInput
            label={t('speedSchedule.limit')}
            value={Math.max(1, Math.round(rule.limit / divisor))}
            onChange={value => onChange({...rule, limit: value * divisor})}
            min={1}
            isIntegerOnly
            isDisabled={isDisabled}
            width={160}
          />
          <ToggleButtonGroup
            label={t('speedSchedule.limit')}
            type="single"
            value={unit}
            onChange={value => {
              if (value === 'kb' || value === 'mb') setUnit(value);
            }}
            isDisabled={isDisabled}>
            <ToggleButton value="kb" label={t('speedSchedule.unitKb')}>
              {t('speedSchedule.unitKb')}
            </ToggleButton>
            <ToggleButton value="mb" label={t('speedSchedule.unitMb')}>
              {t('speedSchedule.unitMb')}
            </ToggleButton>
          </ToggleButtonGroup>
        </HStack>
      )}
    </VStack>
  );
}

/** Speed rules by day and time, shown under Settings → Schedule. */
export function SpeedScheduleCard({settings, update}: SectionProps) {
  const schedule = settings.engine.speedSchedule;
  const isOn = schedule.enabled;

  const setRules = (rules: SpeedRule[]) => update({engine: {speedSchedule: {rules}}});

  return (
    <VStack gap={4}>
      <Heading level={2}>{t('speedSchedule.title')}</Heading>

      <Switch
        label={t('speedSchedule.enable')}
        description={t('speedSchedule.enableHint')}
        value={isOn}
        onChange={value => update({engine: {speedSchedule: {enabled: value}}})}
        labelPosition="start"
        labelSpacing="spread"
        width="100%"
      />

      {schedule.rules.length === 0 && (
        <Text type="supporting" color="secondary">
          {t('speedSchedule.empty')}
        </Text>
      )}
      {schedule.rules.map((rule, index) => (
        <RuleRow
          key={index}
          rule={rule}
          isDisabled={!isOn}
          onChange={next => setRules(schedule.rules.map((item, i) => (i === index ? next : item)))}
          onRemove={() => setRules(schedule.rules.filter((_, i) => i !== index))}
        />
      ))}

      <Button
        label={t('speedSchedule.addRule')}
        variant="secondary"
        icon={<Icon icon={Plus} />}
        isDisabled={!isOn}
        clickAction={() =>
          setRules([
            ...schedule.rules,
            {days: ALL_DAYS, start: minuteToTime(540), end: minuteToTime(1080), limit: MEBIBYTE},
          ])
        }
      />
    </VStack>
  );
}
