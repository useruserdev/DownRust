// SPDX-License-Identifier: GPL-3.0-or-later
import {invoke} from '@tauri-apps/api/core';
import {mockInvoke} from './mock';
import {inTauri} from './runtime';
import type {
  AddRequest,
  AfterQueueAction,
  AppInfo,
  AppSettings,
  ChecksumReport,
  DownloadId,
  DownloadRecord,
  FfmpegInfo,
  HeaderPair,
  ProbeInfo,
  QueueMove,
  TargetSuggestion,
} from './types';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(command, args);
  return (await mockInvoke(command, args)) as T;
}

/** Typed wrappers around every backend command. Arguments use the camelCase names of the Rust side. */
export const api = {
  listDownloads: () => call<DownloadRecord[]>('list_downloads'),
  probeUrl: (url: string, headers: HeaderPair[]) => call<ProbeInfo>('probe_url', {url, headers}),
  suggestTarget: (fileName: string) => call<TargetSuggestion>('suggest_target', {fileName}),
  addDownload: (request: AddRequest) => call<DownloadRecord>('add_download', {request}),
  pauseDownload: (id: DownloadId) => call<null>('pause_download', {id}),
  resumeDownload: (id: DownloadId) => call<null>('resume_download', {id}),
  restartDownload: (id: DownloadId) => call<null>('restart_download', {id}),
  removeDownload: (id: DownloadId, deleteFile: boolean) => call<null>('remove_download', {id, deleteFile}),
  pauseAll: () => call<null>('pause_all'),
  resumeAll: () => call<null>('resume_all'),
  setDownloadConnections: (id: DownloadId, connections: number) =>
    call<null>('set_download_connections', {id, connections}),
  setDownloadSpeedLimit: (id: DownloadId, limit: number | null) =>
    call<null>('set_download_speed_limit', {id, limit}),
  moveInQueue: (id: DownloadId, to: QueueMove) => call<null>('move_in_queue', {id, to}),
  verifyChecksum: (id: DownloadId, expected: string) =>
    call<ChecksumReport>('verify_checksum', {id, expected}),
  refreshAddress: (id: DownloadId, url: string) => call<DownloadRecord>('refresh_address', {id, url}),
  openFile: (id: DownloadId) => call<null>('open_file', {id}),
  showInFolder: (id: DownloadId) => call<null>('show_in_folder', {id}),
  getSettings: () => call<AppSettings>('get_settings'),
  updateSettings: (settings: AppSettings) => call<AppSettings>('update_settings', {settings}),
  getAppInfo: () => call<AppInfo>('get_app_info'),
  frontendReady: () => call<null>('frontend_ready'),
  regenerateBrowserToken: () => call<AppSettings>('regenerate_browser_token'),
  setDropZoneVisible: (visible: boolean) => call<AppSettings>('set_drop_zone_visible', {visible}),
  showMain: () => call<null>('show_main'),
  handleDroppedLinks: (urls: string[]) => call<null>('handle_dropped_links', {urls}),
  setProxyPassword: (password: string | null) => call<null>('set_proxy_password', {password}),
  setSitePassword: (host: string, password: string | null) =>
    call<null>('set_site_password', {host, password}),
  setAfterQueueAction: (action: AfterQueueAction) => call<null>('set_after_queue_action', {action}),
  getAfterQueueAction: () => call<AfterQueueAction>('get_after_queue_action'),
  cancelAfterQueue: () => call<null>('cancel_after_queue'),
  detectFfmpeg: () => call<FfmpegInfo | null>('detect_ffmpeg'),
};
