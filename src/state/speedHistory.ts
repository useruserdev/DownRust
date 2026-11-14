// SPDX-License-Identifier: GPL-3.0-or-later
import {create} from 'zustand';
import type {DownloadId, ProgressBatch} from '../ipc/types';

/** Number of samples kept. One sample arrives per progress batch, so 120 samples cover 60 seconds. */
export const SPEED_SAMPLES = 120;

export interface SpeedHistoryState {
  total: number[];
  byId: Record<DownloadId, number[]>;
  push: (batch: ProgressBatch) => void;
}

function appendSample(series: number[], value: number): number[] {
  const next = [...series, value];
  return next.length > SPEED_SAMPLES ? next.slice(next.length - SPEED_SAMPLES) : next;
}

export const useSpeedHistory = create<SpeedHistoryState>()((set) => ({
  total: [],
  byId: {},
  push: batch =>
    set(state => {
      const byId: Record<DownloadId, number[]> = {};
      for (const item of batch.items) {
        byId[item.id] = appendSample(state.byId[item.id] ?? [], item.speed);
      }
      return {total: appendSample(state.total, batch.totalSpeed), byId};
    }),
}));
