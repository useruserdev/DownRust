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
  /** Options of the download kind as JSON text, for example `{"format":"1080"}` for a video site. */
  options?: string | null;
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

/** Quality of a video site download. `audio` saves only the sound track. */
export type SiteFormat = 'best' | '2160' | '1440' | '1080' | '720' | '480' | 'audio';

/** Settings for video sites downloaded with yt-dlp. */
export interface SiteSettings {
  /** Empty means: look in the tools folder, next to the program, then in PATH. */
  ytdlpPath: string;
  /** Folder of the downloaded yt-dlp program. The app fills it at each start. */
  toolsDir: string;
  autoDownload: boolean;
  autoUpdate: boolean;
  defaultFormat: SiteFormat;
}

/** Details of a video page read with yt-dlp. */
export interface SiteInfo {
  title: string;
  durationSecs: number | null;
  thumbnail: string | null;
  site: string;
  /** Distinct video heights, highest first. */
  heights: number[];
  hasAudio: boolean;
  /** Sanitized title with the `.mp4` extension. */
  fileName: string;
}

/** The yt-dlp program in use and its version. */
export interface SiteToolStatus {
  path: string;
  version: string;
}

/** Settings for torrent downloads. */
export interface TorrentSettings {
  /** Port for incoming peers. 0 lets the system pick a free port. */
  listenPort: number;
  enableDht: boolean;
  enableUpnp: boolean;
  seedAfterFinish: boolean;
  /** Upload limit in bytes per second. null means unlimited. */
  uploadLimit: number | null;
}

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

/** One link collected by the browser extension's link grabber. */
export interface BatchLink {
  url: string;
  text: string;
}

/** Payload of the `external-batch-request` event: links sent by the browser extension. */
export interface ExternalBatchRequest {
  links: BatchLink[];
  referrer: string | null;
  headers: HeaderPair[];
}
