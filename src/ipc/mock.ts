// SPDX-License-Identifier: GPL-3.0-or-later
// Browser stand-in for the Tauri backend. It keeps a small in-memory set of
// downloads, advances them on a timer and emits the same events as the app.
import {categoryForFileName} from '../lib/categories';
import type {
  AddRequest,
  AppInfo,
  UpdateInfo,
  AppSettings,
  ChecksumReport,
  DownloadId,
  DownloadRecord,
  DownloadStatus,
  ProbeInfo,
  ProgressBatch,
  ProgressItem,
  QueueMove,
  SegmentView,
  SiteInfo,
  SiteToolStatus,
} from './types';

const MIB = 1024 * 1024;
const TICK_MS = 500;
const TICK_SECONDS = TICK_MS / 1000;
/** Download records are checkpointed about every 3 seconds, as the engine does. */
const CHECKPOINT_TICKS = 6;
const PER_CONNECTION_BYTES_PER_SECOND = 2.4 * MIB;
const FALLBACK_SIZE = 200 * MIB;
const SAVE_DIR = 'C:\\Users\\Public\\Downloads\\DownRust';

interface MockSegment {
  start: number;
  end: number;
  done: number;
}

interface MockEntry {
  record: DownloadRecord;
  segments: MockSegment[];
  speed: number;
}

type Listener = (payload: unknown) => void;

const listeners = new Map<string, Set<Listener>>();
const entries = new Map<DownloadId, MockEntry>();
let settings: AppSettings = defaultSettings();
let nextId = 1;
let ticker: ReturnType<typeof setInterval> | null = null;
let tickCount = 0;

function defaultSettings(): AppSettings {
  return {
    engine: {
      maxConcurrentDownloads: 2,
      defaultConnections: 8,
      globalSpeedLimit: null,
      minSegmentSize: MIB,
      maxRetries: 5,
      retryBaseDelayMs: 1000,
      retryMaxDelayMs: 30000,
      connectTimeoutSecs: 15,
      readTimeoutSecs: 30,
      checkpointIntervalMs: 3000,
      progressIntervalMs: TICK_MS,
      userAgent: 'DownRust/0.1',
      useSystemProxy: true,
      preallocate: true,
      resumeOnStartup: true,
      defaultDir: SAVE_DIR,
      autoSortByCategory: false,
      categoryDirs: {},
      schedule: {enabled: false, startMinute: 120, stopMinute: 480, days: [true, true, true, true, true, true, true]},
      proxy: {mode: 'system', url: '', username: '', rules: []},
      siteLogins: [],
      scan: {enabled: false, program: '', args: '-Scan -ScanType 3 -File "{file}" -DisableRemediation'},
      media: {ffmpegPath: '', convertToMp4: true, quality: 'best'},
      site: {ytdlpPath: '', toolsDir: '', autoDownload: true, autoUpdate: true, defaultFormat: 'best'},
      torrent: {listenPort: 0, enableDht: true, enableUpnp: true, seedAfterFinish: false, uploadLimit: null},
      speedSchedule: {enabled: false, rules: []},
      autoChecksum: true,
    },
    theme: 'system',
    language: 'system',
    clipboardMonitor: false,
    startMinimized: false,
    launchOnStartup: false,
    closeToTray: true,
    notifyOnComplete: true,
    browserIntegration: false,
    browserPort: 7621,
    browserToken: randomToken(),
    showDropZone: false,
    dropZonePosition: null,
    checkUpdates: true,
    handleMagnetLinks: false,
  };
}

function randomToken(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(16)), b => b.toString(16).padStart(2, '0')).join('');
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function ensureTicker(): void {
  if (ticker === null) {
    ticker = setInterval(tick, TICK_MS);
  }
}

function emit(name: string, payload: unknown): void {
  listeners.get(name)?.forEach(handler => handler(payload));
}

/** Subscribes to a mock event. Returns the unsubscribe function. */
export function mockListen(name: string, handler: Listener): () => void {
  ensureTicker();
  let set = listeners.get(name);
  if (!set) {
    set = new Set();
    listeners.set(name, set);
  }
  set.add(handler);
  return () => {
    listeners.get(name)?.delete(handler);
  };
}

function snapshot(record: DownloadRecord): DownloadRecord {
  return clone(record);
}

function emitUpdated(entry: MockEntry): void {
  emit('download-updated', snapshot(entry.record));
}

function countStatus(status: DownloadStatus): number {
  let count = 0;
  for (const entry of entries.values()) {
    if (entry.record.status === status) count += 1;
  }
  return count;
}

function setStatus(entry: MockEntry, status: DownloadStatus): void {
  entry.record.status = status;
  if (status !== 'downloading') entry.speed = 0;
  emitUpdated(entry);
}

function fallbackName(url: string): string {
  const last = url.split('?')[0].split('/').filter(Boolean).pop();
  if (!last) return 'download.bin';
  try {
    return decodeURIComponent(last);
  } catch {
    return last;
  }
}

/** Splits the file into one segment per connection. Each segment has the same share of the download ratio. */
function buildSegments(total: number, connections: number, downloaded: number): MockSegment[] {
  const count = Math.max(1, connections);
  const ratio = total > 0 ? Math.min(1, downloaded / total) : 0;
  const width = total / count;
  const segments: MockSegment[] = [];
  for (let i = 0; i < count; i += 1) {
    const start = Math.floor(i * width);
    const end = i === count - 1 ? total : Math.floor((i + 1) * width);
    segments.push({start, end, done: Math.round((end - start) * ratio)});
  }
  return segments;
}

function segmentViews(segments: MockSegment[]): SegmentView[] {
  return segments.map(segment => ({
    start: segment.start,
    end: segment.end,
    done: segment.done,
    active: segment.done < segment.end - segment.start,
  }));
}

function downloadedBytes(segments: MockSegment[]): number {
  return segments.reduce((sum, segment) => sum + segment.done, 0);
}

function makeEntry(record: DownloadRecord, downloaded: number): MockEntry {
  const total = record.totalSize ?? 0;
  return {record, segments: buildSegments(total, record.connections, downloaded), speed: 0};
}

function nextQueuePosition(): number {
  let max = 0;
  for (const entry of entries.values()) {
    max = Math.max(max, entry.record.queuePosition);
  }
  return max + 1;
}

/** Starts queued downloads in queue order while there are free slots. */
function fillSlots(): void {
  let active = countStatus('downloading');
  const limit = settings.engine.maxConcurrentDownloads;
  const queued = [...entries.values()]
    .filter(entry => entry.record.status === 'queued')
    .sort((a, b) => a.record.queuePosition - b.record.queuePosition);
  for (const entry of queued) {
    if (active >= limit) break;
    setStatus(entry, 'downloading');
    active += 1;
  }
}

function completeEntry(entry: MockEntry): void {
  const total = entry.record.totalSize ?? 0;
  entry.record.downloaded = total;
  entry.record.completedAt = Date.now();
  entry.segments = entry.segments.map(segment => ({...segment, done: segment.end - segment.start}));
  entry.record.status = 'completed';
  entry.speed = 0;
  emit('download-completed', snapshot(entry.record));
}

function tick(): void {
  tickCount += 1;
  const downloading = [...entries.values()].filter(entry => entry.record.status === 'downloading');

  for (const entry of downloading) {
    const jitter = 0.85 + Math.random() * 0.3;
    let speed = PER_CONNECTION_BYTES_PER_SECOND * jitter * entry.record.connections;
    if (entry.record.speedLimit !== null) speed = Math.min(speed, entry.record.speedLimit);
    entry.speed = speed;
  }

  const rawTotal = downloading.reduce((sum, entry) => sum + entry.speed, 0);
  const globalLimit = settings.engine.globalSpeedLimit;
  const scale = globalLimit !== null && rawTotal > globalLimit ? globalLimit / rawTotal : 1;

  for (const entry of downloading) {
    entry.speed *= scale;
    advance(entry, entry.speed * TICK_SECONDS);
  }

  const finished = downloading.filter(entry => entry.record.status === 'downloading' && isFinished(entry));
  for (const entry of finished) {
    completeEntry(entry);
  }

  const items: ProgressItem[] = [];
  for (const entry of entries.values()) {
    if (entry.record.status !== 'downloading') continue;
    const total = entry.record.totalSize;
    const remaining = total === null ? null : total - entry.record.downloaded;
    items.push({
      id: entry.record.id,
      downloaded: entry.record.downloaded,
      total,
      speed: entry.speed,
      etaSecs: remaining !== null && entry.speed > 0 ? remaining / entry.speed : null,
      connections: entry.record.connections,
      phase: 'downloading',
      segments: segmentViews(entry.segments),
    });
  }
  const batch: ProgressBatch = {items, totalSpeed: items.reduce((sum, item) => sum + item.speed, 0)};
  emit('download-progress', batch);

  if (tickCount % CHECKPOINT_TICKS === 0) {
    for (const entry of entries.values()) {
      if (entry.record.status === 'downloading') emitUpdated(entry);
    }
  }

  if (finished.length > 0) fillSlots();
}

function advance(entry: MockEntry, bytes: number): void {
  const active = entry.segments.filter(segment => segment.done < segment.end - segment.start);
  if (active.length === 0) return;
  const share = bytes / active.length;
  for (const segment of active) {
    segment.done = Math.min(segment.end - segment.start, segment.done + share);
  }
  entry.record.downloaded = downloadedBytes(entry.segments);
}

function isFinished(entry: MockEntry): boolean {
  return entry.segments.every(segment => segment.done >= segment.end - segment.start);
}

function sampleRecord(
  fileName: string,
  url: string,
  status: DownloadStatus,
  totalSize: number,
  downloadedRatio: number,
  connections: number,
  queuePosition: number,
  createdMinutesAgo: number,
  error: string | null,
): MockEntry {
  const id = nextId;
  nextId += 1;
  const downloaded = Math.round(totalSize * downloadedRatio);
  const record: DownloadRecord = {
    id,
    url,
    finalUrl: url,
    referrer: null,
    fileName,
    saveDir: SAVE_DIR,
    category: categoryForFileName(fileName),
    status,
    totalSize,
    downloaded,
    resumable: true,
    connections,
    speedLimit: null,
    etag: `"sample-${id}"`,
    lastModified: null,
    contentType: 'application/octet-stream',
    error,
    queuePosition,
    headers: [],
    createdAt: Date.now() - createdMinutesAgo * 60_000,
    kind: 'file',
    completedAt: status === 'completed' ? Date.now() - 60_000 : null,
  };
  const entry = makeEntry(record, downloaded);
  if (status === 'completed') {
    entry.segments = entry.segments.map(segment => ({...segment, done: segment.end - segment.start}));
    record.downloaded = totalSize;
  }
  return entry;
}

function seedSamples(): void {
  const samples: MockEntry[] = [
    sampleRecord('ubuntu-24.04.1-desktop-amd64.iso', 'https://mirror.example.org/ubuntu/24.04.1/ubuntu-24.04.1-desktop-amd64.iso', 'downloading', 5.8 * 1024 * MIB, 0.42, 8, 0, 14, null),
    sampleRecord('lecture-07-distributed-systems.mp4', 'https://media.example.org/courses/cs-410/lecture-07-distributed-systems.mp4', 'downloading', 1.2 * 1024 * MIB, 0.18, 4, 0, 9, null),
    sampleRecord('field-recordings-vol2.flac', 'https://archive.example.org/audio/field-recordings-vol2.flac', 'queued', 480 * MIB, 0, 8, 1, 6, null),
    sampleRecord('rust-book-2026-edition.pdf', 'https://docs.example.org/rust/rust-book-2026-edition.pdf', 'paused', 38 * MIB, 0.6, 4, 2, 30, null),
    sampleRecord('photo-archive-2025.zip', 'https://files.example.org/photos/photo-archive-2025.zip', 'completed', 2.1 * 1024 * MIB, 1, 8, 3, 240, null),
    sampleRecord('nightly-build-0.9.3.tar.gz', 'https://builds.example.org/nightly/nightly-build-0.9.3.tar.gz', 'error', 312 * MIB, 0.09, 8, 4, 60, 'HTTP 503 Service Unavailable'),
  ];
  const hlsSample = sampleRecord('lecture-08-stream.mp4', 'https://media.example.org/hls/lecture-08/master.m3u8', 'downloading', 900 * MIB, 0.35, 4, 0, 4, null);
  hlsSample.record.kind = 'hls';
  samples.push(hlsSample);
  for (const entry of samples) {
    entries.set(entry.record.id, entry);
  }
}

seedSamples();

function probe(url: string): ProbeInfo {
  const fileName = fallbackName(url);
  return {
    url,
    finalUrl: url,
    fileName,
    totalSize: FALLBACK_SIZE,
    resumable: true,
    contentType: 'application/octet-stream',
    etag: '"probe-etag"',
    lastModified: null,
    category: categoryForFileName(fileName),
  };
}

function addDownload(request: AddRequest): DownloadRecord {
  const fileName = request.fileName ?? request.probe?.fileName ?? fallbackName(request.url);
  const id = nextId;
  nextId += 1;
  const connections = request.connections ?? settings.engine.defaultConnections;
  const activeCount = countStatus('downloading');
  let status: DownloadStatus = 'paused';
  if (request.start) {
    status = activeCount < settings.engine.maxConcurrentDownloads ? 'downloading' : 'queued';
  }
  const record: DownloadRecord = {
    id,
    url: request.url,
    finalUrl: request.probe?.finalUrl ?? request.url,
    referrer: request.referrer,
    fileName,
    saveDir: request.saveDir ?? settings.engine.defaultDir,
    category: categoryForFileName(fileName),
    status,
    totalSize: request.probe?.totalSize ?? FALLBACK_SIZE,
    downloaded: 0,
    resumable: request.probe?.resumable ?? true,
    connections,
    speedLimit: request.speedLimit,
    etag: request.probe?.etag ?? null,
    lastModified: request.probe?.lastModified ?? null,
    contentType: request.probe?.contentType ?? null,
    error: null,
    queuePosition: nextQueuePosition(),
    headers: clone(request.headers),
    createdAt: Date.now(),
    completedAt: null,
    expectedSha256: request.expectedSha256 ?? null,
    checksum: null,
    kind: request.kind ?? 'file',
  };
  const entry = makeEntry(record, 0);
  entries.set(id, entry);
  emit('download-added', snapshot(record));
  return snapshot(record);
}

function requireEntry(id: DownloadId): MockEntry {
  const entry = entries.get(id);
  if (!entry) throw new Error(`Download ${id} not found`);
  return entry;
}

function resetSegments(entry: MockEntry): void {
  const total = entry.record.totalSize ?? 0;
  entry.segments = buildSegments(total, entry.record.connections, 0);
  entry.record.downloaded = 0;
}

function snapshotAll(): DownloadRecord[] {
  return [...entries.values()].map(entry => snapshot(entry.record)).sort((a, b) => a.id - b.id);
}

function setConnections(id: DownloadId, connections: number): void {
  const entry = requireEntry(id);
  entry.record.connections = connections;
  entry.segments = buildSegments(entry.record.totalSize ?? 0, connections, entry.record.downloaded);
  emitUpdated(entry);
}

function remove(id: DownloadId): void {
  entries.delete(id);
  emit('download-removed', {id});
  fillSlots();
}

/** Queued downloads in queue order (queue position, then id). */
function queuedEntries(): MockEntry[] {
  return [...entries.values()]
    .filter(entry => entry.record.status === 'queued')
    .sort((a, b) => a.record.queuePosition - b.record.queuePosition || a.record.id - b.record.id);
}

/** Moves a queued download. The queue positions of the queued downloads are kept as a set. */
function moveInQueue(id: DownloadId, to: QueueMove): void {
  const entry = requireEntry(id);
  if (entry.record.status !== 'queued') throw new Error('Only queued downloads can be moved in the queue.');
  const queued = queuedEntries();
  const slots = queued.map(item => item.record.queuePosition);
  const index = queued.indexOf(entry);
  const last = queued.length - 1;
  let target = index;
  if (to === 'up') target = Math.max(0, index - 1);
  else if (to === 'down') target = Math.min(last, index + 1);
  else if (to === 'top') target = 0;
  else target = last;
  if (target === index) return;
  const order = [...queued];
  order.splice(index, 1);
  order.splice(target, 0, entry);
  order.forEach((item, i) => {
    if (item.record.queuePosition !== slots[i]) {
      item.record.queuePosition = slots[i];
      emitUpdated(item);
    }
  });
}

/** The browser preview has no file to hash. It reports the expected value as the actual one. */
function verifyChecksum(id: DownloadId, expected: string): ChecksumReport {
  const entry = requireEntry(id);
  if (entry.record.status !== 'completed') throw new Error('Only completed downloads can be verified.');
  const value = expected.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error('The SHA-256 must be 64 hexadecimal characters.');
  entry.record.expectedSha256 = value;
  entry.record.checksum = 'ok';
  emitUpdated(entry);
  return {actual: value, matches: true};
}

function refreshAddress(id: DownloadId, url: string): DownloadRecord {
  const entry = requireEntry(id);
  if (!['paused', 'error', 'queued'].includes(entry.record.status)) {
    throw new Error('The address can only be changed for a paused, failed or queued download.');
  }
  const next = url.trim();
  if (probe(next).totalSize !== entry.record.totalSize) throw new Error('The new address points to a different file.');
  entry.record.url = next;
  entry.record.finalUrl = next;
  emitUpdated(entry);
  return snapshot(entry.record);
}

/** Dispatches a command the same way the Tauri backend would. Resolves with the command result. */
export async function mockInvoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
  ensureTicker();
  const a = args ?? {};
  switch (command) {
    case 'list_downloads':
      return snapshotAll();
    case 'probe_url':
      return probe(String(a.url));
    case 'suggest_target': {
      const fileName = String(a.fileName);
      return {category: categoryForFileName(fileName), saveDir: settings.engine.defaultDir, fileName};
    }
    case 'add_download':
      return addDownload(a.request as AddRequest);
    case 'pause_download': {
      const entry = requireEntry(a.id as DownloadId);
      if (entry.record.status === 'downloading' || entry.record.status === 'queued') {
        setStatus(entry, 'paused');
        fillSlots();
      }
      return null;
    }
    case 'resume_download': {
      const entry = requireEntry(a.id as DownloadId);
      if (entry.record.status === 'paused' || entry.record.status === 'error') {
        entry.record.error = null;
        setStatus(entry, 'queued');
        fillSlots();
      }
      return null;
    }
    case 'restart_download': {
      const entry = requireEntry(a.id as DownloadId);
      resetSegments(entry);
      entry.record.error = null;
      entry.record.completedAt = null;
      setStatus(entry, 'queued');
      fillSlots();
      return null;
    }
    case 'remove_download':
      remove(a.id as DownloadId);
      return null;
    case 'pause_all':
      for (const entry of entries.values()) {
        if (entry.record.status === 'downloading' || entry.record.status === 'queued') setStatus(entry, 'paused');
      }
      fillSlots();
      return null;
    case 'resume_all':
      for (const entry of entries.values()) {
        if (entry.record.status === 'paused') setStatus(entry, 'queued');
      }
      fillSlots();
      return null;
    case 'set_download_connections':
      setConnections(a.id as DownloadId, Number(a.connections));
      return null;
    case 'set_download_speed_limit': {
      const entry = requireEntry(a.id as DownloadId);
      entry.record.speedLimit = (a.limit as number | null) ?? null;
      emitUpdated(entry);
      return null;
    }
    case 'move_in_queue':
      moveInQueue(a.id as DownloadId, a.to as QueueMove);
      return null;
    case 'verify_checksum':
      return verifyChecksum(a.id as DownloadId, String(a.expected));
    case 'refresh_address':
      return refreshAddress(a.id as DownloadId, String(a.url));
    case 'open_file':
    case 'show_in_folder':
    case 'frontend_ready':
      return null;
    case 'get_settings':
      return clone(settings);
    case 'update_settings': {
      settings = clone(a.settings as AppSettings);
      emit('settings-changed', clone(settings));
      fillSlots();
      return clone(settings);
    }
    case 'detect_ffmpeg':
      return null;
    case 'check_update': {
      const info: UpdateInfo = {
        currentVersion: '0.2.0',
        available: false,
        version: null,
        notes: null,
        date: null,
        source: 'updater',
      };
      return info;
    }
    case 'install_update':
    case 'open_link':
    case 'set_magnet_handler':
      return null;
    case 'site_info': {
      const title = 'Browser preview video';
      const info: SiteInfo = {
        title,
        durationSecs: 120,
        thumbnail: null,
        site: 'Preview',
        heights: [1080, 720, 480],
        hasAudio: true,
        fileName: `${title}.mp4`,
      };
      return info;
    }
    case 'site_tool_status':
      return null;
    case 'site_tool_update': {
      const status: SiteToolStatus = {path: 'C:\\Users\\Public\\AppData\\Roaming\\DownRust\\tools\\yt-dlp.exe', version: '2026.01.01'};
      return status;
    }
    case 'import_download_list':
      return [] as string[];
    case 'get_app_info': {
      const info: AppInfo = {
        version: '0.2.0',
        dataDir: 'C:\\Users\\Public\\AppData\\Roaming\\DownRust',
        logDir: 'C:\\Users\\Public\\AppData\\Roaming\\DownRust\\logs',
        portable: false,
      };
      return info;
    }
    case 'regenerate_browser_token':
      settings = {...settings, browserToken: randomToken()};
      emit('settings-changed', clone(settings));
      return clone(settings);
    case 'set_drop_zone_visible':
      settings = {...settings, showDropZone: Boolean(a.visible)};
      emit('settings-changed', clone(settings));
      return clone(settings);
    case 'set_proxy_password':
    case 'set_site_password':
    case 'set_after_queue_action':
    case 'cancel_after_queue':
      return null;
    case 'get_after_queue_action':
      return 'none';
    case 'show_main':
    case 'handle_dropped_links':
      return null;
    default:
      throw new Error(`Unknown command: ${command}`);
  }
}
