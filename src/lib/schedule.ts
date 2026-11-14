// SPDX-License-Identifier: GPL-3.0-or-later
import type {Schedule} from '../ipc/types';
import {t} from '../i18n';
import type {MessageKey} from '../i18n';

const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'] as const;
type DayName = (typeof DAY_NAMES)[number];

const DAY_KEYS: Record<DayName, MessageKey> = {
  Mon: 'day.mon',
  Tue: 'day.tue',
  Wed: 'day.wed',
  Thu: 'day.thu',
  Fri: 'day.fri',
  Sat: 'day.sat',
  Sun: 'day.sun',
};

const MINUTES_PER_DAY = 24 * 60;

/** Short names of the days, Monday first. They are the stored values; use dayLabel to show them. */
export const WEEKDAY_NAMES = DAY_NAMES;

/** The short day name in the screen language. */
export function dayLabel(name: DayName): string {
  return t(DAY_KEYS[name]);
}

/** Formats minutes after midnight as HH:MM. */
export function minuteToTime(minute: number): string {
  const clamped = Math.min(MINUTES_PER_DAY - 1, Math.max(0, Math.round(minute)));
  const hours = Math.floor(clamped / 60);
  const minutes = clamped % 60;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`;
}

/** Parses HH:MM into minutes after midnight. Returns null when the text is not a valid time. */
export function timeToMinute(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value.trim());
  if (!match) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  if (hours > 23 || minutes > 59) return null;
  return hours * 60 + minutes;
}

/**
 * Mirrors the rule the engine uses. Disabled means always open.
 * Equal start and stop mean the whole selected day. A start before the stop is a daytime window.
 * A start after the stop crosses midnight, and the start day counts for the part after the start.
 */
export function isWindowOpen(schedule: Schedule, weekdayFromMonday: number, minute: number): boolean {
  if (!schedule.enabled) return true;
  const day = ((weekdayFromMonday % 7) + 7) % 7;
  const previous = (day + 6) % 7;
  if (schedule.startMinute === schedule.stopMinute) return schedule.days[day];
  if (schedule.startMinute < schedule.stopMinute) {
    return schedule.days[day] && minute >= schedule.startMinute && minute < schedule.stopMinute;
  }
  return (
    (schedule.days[day] && minute >= schedule.startMinute) ||
    (schedule.days[previous] && minute < schedule.stopMinute)
  );
}

/** Reports whether downloads may run at the given local date and time. */
export function isOpenNow(schedule: Schedule, date: Date): boolean {
  const weekday = (date.getDay() + 6) % 7;
  const minute = date.getHours() * 60 + date.getMinutes();
  return isWindowOpen(schedule, weekday, minute);
}

/** One line that describes when downloads run, in the screen language. */
export function scheduleSummary(schedule: Schedule): string {
  const start = minuteToTime(schedule.startMinute);
  const stop = minuteToTime(schedule.stopMinute);
  const selected = schedule.days.map((on, index) => (on ? dayLabel(DAY_NAMES[index]) : null)).filter(Boolean);
  const allDays = selected.length === 7;
  const daysText = allDays
    ? t('schedule.everyDay')
    : selected.length === 0
      ? t('schedule.noDay')
      : t('schedule.onDays', {days: selected.join(', ')});
  if (schedule.startMinute === schedule.stopMinute) {
    return t('schedule.allDay', {days: daysText});
  }
  if (schedule.startMinute > schedule.stopMinute) {
    return t('schedule.overnight', {start, stop, days: daysText});
  }
  return t('schedule.window', {start, stop, days: daysText});
}
