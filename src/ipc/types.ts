// SPDX-License-Identifier: GPL-3.0-or-later

export type DownloadId = number;
export type DownloadStatus = 'queued' | 'downloading' | 'paused' | 'completed' | 'error';
export type Category = 'video' | 'music' | 'archives' | 'programs' | 'documents' | 'other';
export type Phase = 'connecting' | 'downloading' | 'finalizing';
export type ThemeMode = 'system' | 'dark' | 'light';
export type QueueMove = 'up' | 'down' | 'top' | 'bottom';
export type DownloadKind = 'file' | 'hls' | 'dash' | 'site' | 'torrent' | 'ftp';
export type MediaQuality = 'best' | 'worst';

export interface MediaSettings {
  /** Empty means: look next to the program, then in PATH. */
  ffmpegPath: string;
  convertToMp4: boolean;
  quality: MediaQuality;
}

export interface FfmpegInfo {
  path: string;
  version: string;
}
export type ChecksumState = 'verifying' | 'ok' | 'mismatch';
export type ScanState = 'scanning' | 'clean' | 'threat' | 'failed';
export type AfterQueueAction = 'none' | 'exit' | 'sleep' | 'hibernate' | 'shutdown';
export type ProxyMode = 'system' | 'direct' | 'manual';
export type ProxyRuleAction = 'proxy' | 'direct';

/** Event name: countdown started after the queue emptied. Payload: AfterQueueCountdown. */
export const AFTER_QUEUE_COUNTDOWN_EVENT = 'after-queue-countdown';
/** Event name: the countdown was cancelled. No payload. */
export const AFTER_QUEUE_CANCELLED_EVENT = 'after-queue-cancelled';

export interface HeaderPair {
  name: string;
  value: string;
}

export interface DownloadRecord {
  id: DownloadId;
  url: string;
  finalUrl: string | null;
  referrer: string | null;
  fileName: string;
  saveDir: string;
  category: Category;
  status: DownloadStatus;
  totalSize: number | null;
  downloaded: number;
  resumable: boolean;
  connections: number;
  speedLimit: number | null;
  etag: string | null;
  lastModified: string | null;
  contentType: string | null;
  error: string | null;
  queuePosition: number;
  headers: HeaderPair[];
  createdAt: number;
  completedAt: number | null;
  expectedSha256?: string | null;
  checksum?: ChecksumState | null;
  scan?: ScanState | null;
  kind?: DownloadKind;
  /** Other addresses of the same file. */
  mirrors?: string[];
  /** Options of the download kind as JSON text. */
  options?: string | null;
}

export interface ChecksumReport {
  actual: string;
  matches: boolean;
}

export interface ProbeInfo {
  url: string;
  finalUrl: string;
  fileName: string;
  totalSize: number | null;
  resumable: boolean;
  contentType: string | null;
  etag: string | null;
  lastModified: string | null;
  category: Category;
}

export interface TargetSuggestion {
  category: Category;
  saveDir: string;
  fileName: string;
}

export interface AddRequest {
  url: string;
  fileName: string | null;
  saveDir: string | null;
  connections: number | null;
  speedLimit: number | null;
  start: boolean;
  referrer: string | null;
  headers: HeaderPair[];
  probe: ProbeInfo | null;
  expectedSha256?: string | null;
  /** Omitted or null: the kind is detected from the address and the response type. */
  kind?: DownloadKind | null;
}

export interface SegmentView {
  start: number;
  end: number;
  done: number;
  active: boolean;
}

export interface ProgressItem {
  id: DownloadId;
  downloaded: number;
  total: number | null;
  speed: number;
  etaSecs: number | null;
  connections: number;
  phase: Phase;
  segments: SegmentView[];
}

export interface ProgressBatch {
  items: ProgressItem[];
  totalSpeed: number;
}

export interface EngineSettings {
  maxConcurrentDownloads: number;
  defaultConnections: number;
  globalSpeedLimit: number | null;
  minSegmentSize: number;
  maxRetries: number;
  retryBaseDelayMs: number;
  retryMaxDelayMs: number;
  connectTimeoutSecs: number;
  readTimeoutSecs: number;
  checkpointIntervalMs: number;
  progressIntervalMs: number;
  userAgent: string;
  useSystemProxy: boolean;
  preallocate: boolean;
  resumeOnStartup: boolean;
  defaultDir: string;
  autoSortByCategory: boolean;
  categoryDirs: Partial<Record<Category, string>>;
  schedule: Schedule;
  proxy: ProxySettings;
  siteLogins: SiteLogin[];
  scan: ScanSettings;
  media: MediaSettings;
  site: SiteSettings;
  torrent: TorrentSettings;
  speedSchedule: SpeedSchedule;
  /** Looks for a published SHA-256 next to a finished file and checks the file against it. */
  autoChecksum: boolean;
}

/** Settings for video sites downloaded with yt-dlp. */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface SiteSettings {}

/** Settings for torrent downloads. */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface TorrentSettings {}

/** Speed limits that change with the time of day. */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface SpeedSchedule {}

export interface ProxyRule {
  host: string;
  action: ProxyRuleAction;
}

export interface ProxySettings {
  mode: ProxyMode;
  /** Address such as http://host:port, socks5://host:port or socks5h://host:port. */
  url: string;
  username: string;
  rules: ProxyRule[];
}

export interface SiteLogin {
  host: string;
  username: string;
}

export interface ScanSettings {
  enabled: boolean;
  /** Empty selects the Windows Defender command line scanner. */
  program: string;
  /** `{file}` is replaced with the path of the finished file. */
  args: string;
}

export interface AfterQueueCountdown {
  action: AfterQueueAction;
  seconds: number;
}

export interface Schedule {
  enabled: boolean;
  startMinute: number;
  stopMinute: number;
  days: [boolean, boolean, boolean, boolean, boolean, boolean, boolean];
}

export interface AppSettings {
  engine: EngineSettings;
  theme: ThemeMode;
  language: 'system' | 'en' | 'ru';
  clipboardMonitor: boolean;
  startMinimized: boolean;
  launchOnStartup: boolean;
  closeToTray: boolean;
  notifyOnComplete: boolean;
  browserIntegration: boolean;
  browserPort: number;
  browserToken: string;
  showDropZone: boolean;
  dropZonePosition: [number, number] | null;
  checkUpdates: boolean;
  handleMagnetLinks: boolean;
}

/** Result of an update check. `source` is `github` when the GitHub release was read instead. */
export interface UpdateInfo {
  currentVersion: string;
  available: boolean;
  version: string | null;
  notes: string | null;
  date: string | null;
  source: 'updater' | 'github';
}

/** Payload of the `update-progress` event while the update downloads. */
export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

/** Payload of the `open-batch-dialog` event: the dropped links, one per line. */
export interface OpenBatchPayload {
  text: string;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  logDir: string;
  portable: boolean;
}

export interface ExternalAddRequest {
  url: string;
  fileName: string | null;
  referrer: string | null;
  headers: HeaderPair[];
  kind?: DownloadKind | null;
}
