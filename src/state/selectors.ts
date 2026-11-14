// SPDX-License-Identifier: GPL-3.0-or-later
import {useShallow} from 'zustand/react/shallow';
import type {Category, DownloadRecord} from '../ipc/types';
import type {DownloadFilter, DownloadsState, StatusFilterValue} from './downloads';
import {useDownloads} from './downloads';

export type StatusCounts = Record<StatusFilterValue, number>;

const STATUS_PREDICATES: Record<StatusFilterValue, (record: DownloadRecord) => boolean> = {
  all: () => true,
  active: record => record.status === 'downloading',
  queued: record => record.status === 'queued',
  paused: record => record.status === 'paused',
  completed: record => record.status === 'completed',
  failed: record => record.status === 'error',
};

function matchesFilter(record: DownloadRecord, filter: DownloadFilter): boolean {
  if (filter.kind === 'category') return record.category === filter.value;
  return STATUS_PREDICATES[filter.value](record);
}

function matchesSearch(record: DownloadRecord, search: string): boolean {
  const query = search.trim().toLowerCase();
  if (query === '') return true;
  return record.fileName.toLowerCase().includes(query) || record.url.toLowerCase().includes(query);
}

/** Rows shown in the table: filtered by status or category, then by search text, newest first. */
export function selectVisibleRows(state: DownloadsState): DownloadRecord[] {
  return Object.values(state.records)
    .filter(record => matchesFilter(record, state.filter) && matchesSearch(record, state.search))
    .sort((a, b) => b.createdAt - a.createdAt);
}

export function selectStatusCounts(state: DownloadsState): StatusCounts {
  const counts: StatusCounts = {all: 0, active: 0, queued: 0, paused: 0, completed: 0, failed: 0};
  for (const record of Object.values(state.records)) {
    counts.all += 1;
    for (const key of Object.keys(STATUS_PREDICATES) as StatusFilterValue[]) {
      if (key !== 'all' && STATUS_PREDICATES[key](record)) counts[key] += 1;
    }
  }
  return counts;
}

export function selectCategoryCounts(state: DownloadsState): Record<Category, number> {
  const counts: Record<Category, number> = {video: 0, music: 0, archives: 0, programs: 0, documents: 0, other: 0};
  for (const record of Object.values(state.records)) {
    counts[record.category] += 1;
  }
  return counts;
}

export function useVisibleRows(): DownloadRecord[] {
  return useDownloads(useShallow(selectVisibleRows));
}

export function useStatusCounts(): StatusCounts {
  return useDownloads(useShallow(selectStatusCounts));
}

export function useCategoryCounts(): Record<Category, number> {
  return useDownloads(useShallow(selectCategoryCounts));
}
