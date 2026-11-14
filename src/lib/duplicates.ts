// SPDX-License-Identifier: GPL-3.0-or-later
import type {DownloadRecord} from '../ipc/types';

export type DuplicateMatch = {kind: 'link'; record: DownloadRecord} | {kind: 'file'; record: DownloadRecord};

export interface DuplicateProbe {
  url: string;
  fileName?: string;
  totalSize?: number | null;
}

/** Trims the address, drops the #fragment and lowercases the scheme and host. */
export function normalizeUrl(url: string): string {
  const trimmed = url.trim();
  try {
    const parsed = new URL(trimmed);
    parsed.hash = '';
    return parsed.href;
  } catch {
    return trimmed;
  }
}

/**
 * Finds a record that makes the new download a duplicate.
 * A link duplicate has the same normalized address. A file duplicate is a completed record with the same
 * file name (case-insensitive) and the same total size. Link matches win over file matches.
 */
export function findDuplicate(records: readonly DownloadRecord[], probe: DuplicateProbe): DuplicateMatch | null {
  const target = normalizeUrl(probe.url);
  const sameLink = records.find(record => normalizeUrl(record.url) === target);
  if (sameLink) return {kind: 'link', record: sameLink};

  const name = probe.fileName?.trim().toLowerCase();
  if (!name || probe.totalSize === undefined || probe.totalSize === null) return null;
  const sameFile = records.find(
    record =>
      record.status === 'completed' &&
      record.fileName.toLowerCase() === name &&
      record.totalSize === probe.totalSize,
  );
  return sameFile ? {kind: 'file', record: sameFile} : null;
}
