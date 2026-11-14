// SPDX-License-Identifier: GPL-3.0-or-later
import {localeTag, t} from '../i18n';

const UNIT_KEYS = ['units.b', 'units.kb', 'units.mb', 'units.gb', 'units.tb'] as const;
const EMPTY = '-';

function formatNumber(value: number, digits: number): string {
  return new Intl.NumberFormat(localeTag(), {minimumFractionDigits: digits, maximumFractionDigits: digits}).format(value);
}

function pad2(value: number): string {
  return String(value).padStart(2, '0');
}

/** Formats a byte count with base-1024 units, in the screen language (decimal comma in Russian). */
export function humanBytes(bytes: number | null): string {
  if (bytes === null || !Number.isFinite(bytes)) return EMPTY;
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNIT_KEYS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${formatNumber(Math.round(value), 0)} ${t(UNIT_KEYS[unit])}`;
  const digits = value < 10 ? 2 : value < 100 ? 1 : 0;
  return `${formatNumber(value, digits)} ${t(UNIT_KEYS[unit])}`;
}

/** Formats a transfer rate in bytes per second. */
export function humanSpeed(bytesPerSecond: number | null): string {
  if (bytesPerSecond === null || !Number.isFinite(bytesPerSecond)) return EMPTY;
  return `${humanBytes(bytesPerSecond)}${t('units.perSecond')}`;
}

/** Formats a remaining time in seconds: seconds, minutes, hours with minutes, or days with hours. */
export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return EMPTY;
  const total = Math.round(seconds);
  if (total < 60) return t('eta.seconds', {n: total});
  if (total < 3600) return t('eta.minutes', {n: Math.floor(total / 60)});
  if (total < 86400) {
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    return t('eta.hoursMinutes', {h, m: pad2(m)});
  }
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  return t('eta.daysHours', {d, h});
}

/** Returns a whole percentage, or null when the total size is unknown. */
export function formatPercent(downloaded: number, total: number | null): number | null {
  if (total === null || total <= 0) return null;
  return Math.min(100, Math.floor((downloaded / total) * 100));
}

/** Formats a millisecond timestamp in the local date and time. */
export function formatDateTime(epochMs: number | null): string {
  if (epochMs === null) return EMPTY;
  return new Date(epochMs).toLocaleString(localeTag());
}
