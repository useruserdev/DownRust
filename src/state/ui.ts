// SPDX-License-Identifier: GPL-3.0-or-later
import {create} from 'zustand';
import type {AfterQueueAction, AfterQueueCountdown, DownloadId, DownloadKind, HeaderPair} from '../ipc/types';

export type ViewName = 'downloads' | 'settings' | 'stats';

/** Dialog that asks for a new address of a download. */
export interface AddressDialogState {
  open: boolean;
  id: DownloadId | null;
}

export interface AddPrefill {
  url: string;
  fileName: string | null;
  referrer: string | null;
  headers: HeaderPair[];
  kind?: DownloadKind | null;
}

export interface Notice {
  id: number;
  type: 'info' | 'error';
  body: string;
}

export interface AddDialogState extends AddPrefill {
  open: boolean;
  /** Increases on every open, so the form restarts with the new prefill even when already open. */
  seq: number;
}

export interface BatchDialogState {
  open: boolean;
  text: string;
  /** Increases on every open, so the form restarts with the new text even when already open. */
  seq: number;
}

export interface UiState {
  view: ViewName;
  addDialog: AddDialogState;
  removeDialog: {open: boolean; ids: DownloadId[]};
  speedDialog: {open: boolean; id: DownloadId | null};
  verifyDialog: {open: boolean; id: DownloadId | null};
  addressDialog: AddressDialogState;
  batchDialog: BatchDialogState;
  afterQueue: AfterQueueAction;
  countdown: AfterQueueCountdown | null;
  clipboardUrl: string | null;
  notifications: Notice[];
  setView: (view: ViewName) => void;
  openAdd: (prefill?: Partial<AddPrefill>) => void;
  closeAdd: () => void;
  openBatch: (text?: string) => void;
  closeBatch: () => void;
  openRemove: (ids: DownloadId | DownloadId[]) => void;
  closeRemove: () => void;
  openSpeedLimit: (id?: DownloadId | null) => void;
  closeSpeedLimit: () => void;
  openVerify: (id: DownloadId) => void;
  closeVerify: () => void;
  openAddress: (id: DownloadId) => void;
  closeAddress: () => void;
  setAfterQueue: (action: AfterQueueAction) => void;
  setCountdown: (countdown: AfterQueueCountdown | null) => void;
  setClipboardUrl: (url: string | null) => void;
  pushNotification: (type: Notice['type'], body: string) => void;
  dismissNotification: (id: number) => void;
}

const EMPTY_PREFILL: AddPrefill = {url: '', fileName: null, referrer: null, headers: []};

let noticeSeq = 0;

export const useUi = create<UiState>()((set) => ({
  view: 'downloads',
  addDialog: {open: false, seq: 0, ...EMPTY_PREFILL},
  removeDialog: {open: false, ids: []},
  speedDialog: {open: false, id: null},
  verifyDialog: {open: false, id: null},
  addressDialog: {open: false, id: null},
  batchDialog: {open: false, text: '', seq: 0},
  afterQueue: 'none',
  countdown: null,
  clipboardUrl: null,
  notifications: [],
  setView: view => set({view}),
  openAdd: prefill =>
    set(state => ({addDialog: {open: true, seq: state.addDialog.seq + 1, ...EMPTY_PREFILL, ...prefill}})),
  closeAdd: () => set(state => ({addDialog: {...state.addDialog, open: false}})),
  openBatch: text =>
    set(state => ({batchDialog: {open: true, text: text ?? '', seq: state.batchDialog.seq + 1}})),
  closeBatch: () => set(state => ({batchDialog: {...state.batchDialog, open: false}})),
  openRemove: ids => set({removeDialog: {open: true, ids: Array.isArray(ids) ? ids : [ids]}}),
  closeRemove: () => set({removeDialog: {open: false, ids: []}}),
  openSpeedLimit: id => set({speedDialog: {open: true, id: id ?? null}}),
  closeSpeedLimit: () => set({speedDialog: {open: false, id: null}}),
  openVerify: id => set({verifyDialog: {open: true, id}}),
  closeVerify: () => set({verifyDialog: {open: false, id: null}}),
  openAddress: id => set({addressDialog: {open: true, id}}),
  closeAddress: () => set({addressDialog: {open: false, id: null}}),
  setAfterQueue: action => set({afterQueue: action}),
  setCountdown: countdown => set({countdown}),
  setClipboardUrl: url => set({clipboardUrl: url}),
  pushNotification: (type, body) => {
    noticeSeq += 1;
    const notice: Notice = {id: noticeSeq, type, body};
    set(state => ({notifications: [...state.notifications, notice]}));
  },
  dismissNotification: id =>
    set(state => ({notifications: state.notifications.filter(notice => notice.id !== id)})),
}));
