// SPDX-License-Identifier: GPL-3.0-or-later
import {invoke} from '@tauri-apps/api/core';
import {CATEGORY_ORDER} from '../lib/categories';
import {useDownloads} from '../state/downloads';
import type {Category, DownloadRecord} from './types';
import {inTauri} from './runtime';

export interface StatsDay {
  /** Local calendar date, YYYY-MM-DD. */
  day: string;
  bytes: number;
  files: number;
}

export interface StatsCategory {
  category: Category;
  bytes: number;
  files: number;
}

export interface StatsSnapshot {
  /** The last 30 local days including today, oldest first, with zero-filled days. */
  days: StatsDay[];
  byCategory: StatsCategory[];
  allTime: {bytes: number; files: number};
}

const DAY_COUNT = 30;
const MIB = 1024 * 1024;
const SAMPLE_SEED = 20260101;
const SAMPLE_MAX_FILES_PER_DAY = 4;
const SAMPLE_MIN_FILE_BYTES = 8 * MIB;
const SAMPLE_FILE_BYTES_SPAN = 600 * MIB;

function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** Formats a date as a local calendar day, YYYY-MM-DD. */
export function localDayKey(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Small deterministic generator, so the browser preview shows the same numbers on every load. */
function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function emptyCategories(): Record<Category, StatsCategory> {
  return Object.fromEntries(
    CATEGORY_ORDER.map((category): [Category, StatsCategory] => [category, {category, bytes: 0, files: 0}]),
  ) as Record<Category, StatsCategory>;
}

/** Builds the statistics from the completed records in the store and a fixed pseudo-random history. */
export function buildSampleStats(records: readonly DownloadRecord[], now: Date = new Date()): StatsSnapshot {
  const random = seededRandom(SAMPLE_SEED);
  const dayKeys: string[] = [];
  for (let offset = DAY_COUNT - 1; offset >= 0; offset -= 1) {
    dayKeys.push(localDayKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset)));
  }
  const days = new Map<string, StatsDay>(dayKeys.map(day => [day, {day, bytes: 0, files: 0}]));
  const categories = emptyCategories();

  const addFile = (day: string | null, category: Category, bytes: number) => {
    const total = categories[category];
    total.bytes += bytes;
    total.files += 1;
    const slot = day === null ? undefined : days.get(day);
    if (slot) {
      slot.bytes += bytes;
      slot.files += 1;
    }
  };

  for (const day of dayKeys) {
    const count = Math.floor(random() * (SAMPLE_MAX_FILES_PER_DAY + 1));
    for (let index = 0; index < count; index += 1) {
      const category = CATEGORY_ORDER[Math.floor(random() * CATEGORY_ORDER.length)];
      const bytes = SAMPLE_MIN_FILE_BYTES + Math.floor(random() * SAMPLE_FILE_BYTES_SPAN);
      addFile(day, category, bytes);
    }
  }

  for (const record of records) {
    if (record.status !== 'completed') continue;
    const finished = new Date(record.completedAt ?? record.createdAt);
    addFile(localDayKey(finished), record.category, record.downloaded);
  }

  const byCategory = CATEGORY_ORDER.map(category => categories[category]);
  const allTime = byCategory.reduce(
    (sum, entry) => ({bytes: sum.bytes + entry.bytes, files: sum.files + entry.files}),
    {bytes: 0, files: 0},
  );
  return {days: [...days.values()], byCategory, allTime};
}

/** Loads the statistics. In Tauri the backend computes them; in the browser preview they are built locally. */
export async function fetchStats(): Promise<StatsSnapshot> {
  if (inTauri) return invoke<StatsSnapshot>('get_stats');
  const records = Object.values(useDownloads.getState().records);
  return buildSampleStats(records);
}
