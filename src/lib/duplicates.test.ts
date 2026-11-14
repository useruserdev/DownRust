// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import type {DownloadRecord, DownloadStatus} from '../ipc/types';
import {findDuplicate, normalizeUrl} from './duplicates';

function record(id: number, url: string, fileName: string, status: DownloadStatus, totalSize: number | null): DownloadRecord {
  return {
    id,
    url,
    finalUrl: url,
    referrer: null,
    fileName,
    saveDir: 'C:\\Downloads',
    category: 'other',
    status,
    totalSize,
    downloaded: 0,
    resumable: true,
    connections: 8,
    speedLimit: null,
    etag: null,
    lastModified: null,
    contentType: null,
    error: null,
    queuePosition: id,
    headers: [],
    createdAt: id,
    completedAt: status === 'completed' ? id : null,
  };
}

describe('normalizeUrl', () => {
  it('trims, drops the fragment and lowercases scheme and host', () => {
    expect(normalizeUrl('  HTTPS://Example.COM/Path/File.zip#part-2 ')).toBe('https://example.com/Path/File.zip');
  });

  it('returns the trimmed text when the address does not parse', () => {
    expect(normalizeUrl('  not a link  ')).toBe('not a link');
  });
});

describe('findDuplicate', () => {
  const records = [
    record(1, 'https://files.example/a/archive.zip', 'archive.zip', 'completed', 1000),
    record(2, 'https://files.example/b/movie.mkv', 'movie.mkv', 'downloading', 5000),
  ];

  it('finds a link already in the list, ignoring case in the host and the fragment', () => {
    const match = findDuplicate(records, {url: 'HTTPS://FILES.example/b/movie.mkv#t=5'});
    expect(match?.kind).toBe('link');
    expect(match?.record.id).toBe(2);
  });

  it('returns null for a new link', () => {
    expect(findDuplicate(records, {url: 'https://files.example/c/new.zip'})).toBeNull();
  });

  it('finds a completed file with the same name and size, ignoring name case', () => {
    const match = findDuplicate(records, {url: 'https://mirror.example/ARCHIVE.zip', fileName: 'Archive.ZIP', totalSize: 1000});
    expect(match?.kind).toBe('file');
    expect(match?.record.id).toBe(1);
  });

  it('does not treat a file as a duplicate when the size differs or the record is not completed', () => {
    expect(findDuplicate(records, {url: 'https://x.example/a', fileName: 'archive.zip', totalSize: 999})).toBeNull();
    expect(findDuplicate(records, {url: 'https://x.example/b', fileName: 'movie.mkv', totalSize: 5000})).toBeNull();
    expect(findDuplicate(records, {url: 'https://x.example/c', fileName: 'archive.zip'})).toBeNull();
  });

  it('prefers a link match over a file match', () => {
    const match = findDuplicate(records, {url: 'https://files.example/a/archive.zip', fileName: 'archive.zip', totalSize: 1000});
    expect(match?.kind).toBe('link');
  });
});
