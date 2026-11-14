// SPDX-License-Identifier: GPL-3.0-or-later
import {create} from 'zustand';
import type {Category, DownloadId, DownloadRecord, ProgressBatch, ProgressItem} from '../ipc/types';

export type StatusFilterValue = 'all' | 'active' | 'queued' | 'paused' | 'completed' | 'failed';

export type DownloadFilter =
  | {kind: 'status'; value: StatusFilterValue}
  | {kind: 'category'; value: Category};

export interface DownloadsState {
  records: Record<DownloadId, DownloadRecord>;
  progress: Record<DownloadId, ProgressItem>;
  selectedId: DownloadId | null;
  checkedIds: ReadonlySet<DownloadId>;
  filter: DownloadFilter;
  search: string;
  totalSpeed: number;
  setAll: (records: DownloadRecord[]) => void;
  upsert: (record: DownloadRecord) => void;
  remove: (id: DownloadId) => void;
  applyProgress: (batch: ProgressBatch) => void;
  select: (id: DownloadId | null) => void;
  setChecked: (ids: DownloadId[], checked: boolean) => void;
  clearChecked: () => void;
  setFilter: (filter: DownloadFilter) => void;
  setSearch: (search: string) => void;
}

function withoutKey<T>(source: Record<number, T>, key: number): Record<number, T> {
  const copy = {...source};
  delete copy[key];
  return copy;
}

function withoutMember(source: ReadonlySet<DownloadId>, id: DownloadId): ReadonlySet<DownloadId> {
  if (!source.has(id)) return source;
  const copy = new Set(source);
  copy.delete(id);
  return copy;
}

const NONE_CHECKED: ReadonlySet<DownloadId> = new Set<DownloadId>();

export const useDownloads = create<DownloadsState>()((set) => ({
  records: {},
  progress: {},
  selectedId: null,
  checkedIds: NONE_CHECKED,
  filter: {kind: 'status', value: 'all'},
  search: '',
  totalSpeed: 0,
  setAll: records =>
    set({
      records: Object.fromEntries(records.map(record => [record.id, record])),
      progress: {},
      checkedIds: NONE_CHECKED,
    }),
  upsert: record =>
    set(state => ({
      records: {...state.records, [record.id]: record},
      progress: record.status === 'downloading' ? state.progress : withoutKey(state.progress, record.id),
    })),
  remove: id =>
    set(state => ({
      records: withoutKey(state.records, id),
      progress: withoutKey(state.progress, id),
      selectedId: state.selectedId === id ? null : state.selectedId,
      checkedIds: withoutMember(state.checkedIds, id),
    })),
  applyProgress: batch =>
    set(state => {
      const progress: Record<DownloadId, ProgressItem> = {};
      for (const item of batch.items) {
        if (state.records[item.id]?.status === 'downloading') {
          progress[item.id] = item;
        }
      }
      return {progress, totalSpeed: batch.totalSpeed};
    }),
  select: id => set({selectedId: id}),
  setChecked: (ids, checked) =>
    set(state => {
      const next = new Set(state.checkedIds);
      for (const id of ids) {
        if (checked) next.add(id);
        else next.delete(id);
      }
      return {checkedIds: next};
    }),
  clearChecked: () => set({checkedIds: NONE_CHECKED}),
  setFilter: filter => set({filter, checkedIds: NONE_CHECKED}),
  setSearch: search => set({search, checkedIds: NONE_CHECKED}),
}));
