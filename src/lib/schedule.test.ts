// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import type {Schedule} from '../ipc/types';
import {isOpenNow, isWindowOpen, minuteToTime, timeToMinute} from './schedule';

const ALL = [true, true, true, true, true, true, true] as Schedule['days'];

function schedule(startMinute: number, stopMinute: number, days = ALL, enabled = true): Schedule {
  return {enabled, startMinute, stopMinute, days};
}

describe('isWindowOpen', () => {
  it('is always open when the schedule is disabled', () => {
    expect(isWindowOpen(schedule(600, 620, ALL, false), 2, 0)).toBe(true);
  });

  it('treats equal start and stop as the whole selected day', () => {
    const days = [false, true, false, false, false, false, false] as Schedule['days'];
    expect(isWindowOpen(schedule(300, 300, days), 1, 0)).toBe(true);
    expect(isWindowOpen(schedule(300, 300, days), 1, 1439)).toBe(true);
    expect(isWindowOpen(schedule(300, 300, days), 2, 600)).toBe(false);
  });

  it('includes the start and excludes the stop in a daytime window', () => {
    expect(isWindowOpen(schedule(120, 480), 0, 120)).toBe(true);
    expect(isWindowOpen(schedule(120, 480), 0, 479)).toBe(true);
    expect(isWindowOpen(schedule(120, 480), 0, 480)).toBe(false);
    expect(isWindowOpen(schedule(120, 480), 0, 119)).toBe(false);
  });

  it('respects the selected days in a daytime window', () => {
    const weekdaysOnly = [true, true, true, true, true, false, false] as Schedule['days'];
    expect(isWindowOpen(schedule(120, 480, weekdaysOnly), 5, 200)).toBe(false);
    expect(isWindowOpen(schedule(120, 480, weekdaysOnly), 4, 200)).toBe(true);
  });

  it('crosses midnight: the start day is open after the start', () => {
    const monday = [true, false, false, false, false, false, false] as Schedule['days'];
    expect(isWindowOpen(schedule(1320, 480, monday), 0, 1400)).toBe(true);
    expect(isWindowOpen(schedule(1320, 480, monday), 0, 100)).toBe(false);
  });

  it('crosses midnight: the part before the stop belongs to the previous day', () => {
    const monday = [true, false, false, false, false, false, false] as Schedule['days'];
    expect(isWindowOpen(schedule(1320, 480, monday), 1, 100)).toBe(true);
    expect(isWindowOpen(schedule(1320, 480, monday), 1, 500)).toBe(false);
    expect(isWindowOpen(schedule(1320, 480, monday), 1, 1400)).toBe(false);
  });
});

describe('isOpenNow', () => {
  it('uses the local weekday and minute of the given date', () => {
    // 2026-11-16 is a Monday. 03:00 local time is inside 02:00-08:00.
    const monday3am = new Date(2026, 10, 16, 3, 0, 0);
    expect(isOpenNow(schedule(120, 480), monday3am)).toBe(true);
    const monday9am = new Date(2026, 10, 16, 9, 0, 0);
    expect(isOpenNow(schedule(120, 480), monday9am)).toBe(false);
  });
});

describe('time helpers', () => {
  it('formats minutes as HH:MM', () => {
    expect(minuteToTime(0)).toBe('00:00');
    expect(minuteToTime(125)).toBe('02:05');
    expect(minuteToTime(1439)).toBe('23:59');
  });

  it('parses HH:MM and rejects invalid text', () => {
    expect(timeToMinute('02:05')).toBe(125);
    expect(timeToMinute('24:00')).toBeNull();
    expect(timeToMinute('7:60')).toBeNull();
    expect(timeToMinute('noon')).toBeNull();
  });
});
