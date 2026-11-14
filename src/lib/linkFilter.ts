// SPDX-License-Identifier: GPL-3.0-or-later
import type {BatchLink} from '../ipc/types';

/** Filter choices of the link list. `all` keeps every link. */
export type LinkFilter = 'all' | 'video' | 'audio' | 'archives' | 'documents' | 'programs' | 'images' | 'other';

/** The type of one link. Every link has exactly one type. */
export type LinkType = Exclude<LinkFilter, 'all'>;

export const LINK_FILTERS: LinkFilter[] = [
  'all',
  'video',
  'audio',
  'archives',
  'documents',
  'programs',
  'images',
  'other',
];

const EXTENSION_TYPE: Record<string, LinkType> = {
  mp4: 'video', mkv: 'video', avi: 'video', mov: 'video', webm: 'video', m4v: 'video',
  flv: 'video', wmv: 'video', ts: 'video', mpg: 'video', mpeg: 'video', '3gp': 'video',
  mp3: 'audio', flac: 'audio', wav: 'audio', ogg: 'audio', oga: 'audio', m4a: 'audio',
  aac: 'audio', opus: 'audio', wma: 'audio',
  zip: 'archives', rar: 'archives', '7z': 'archives', tar: 'archives', gz: 'archives',
  tgz: 'archives', bz2: 'archives', xz: 'archives', iso: 'archives',
  pdf: 'documents', doc: 'documents', docx: 'documents', odt: 'documents', rtf: 'documents',
  txt: 'documents', epub: 'documents', xls: 'documents', xlsx: 'documents', ods: 'documents',
  ppt: 'documents', pptx: 'documents', odp: 'documents', csv: 'documents', md: 'documents',
  exe: 'programs', msi: 'programs', dmg: 'programs', pkg: 'programs', appimage: 'programs',
  deb: 'programs', rpm: 'programs', apk: 'programs',
  jpg: 'images', jpeg: 'images', png: 'images', gif: 'images', webp: 'images', bmp: 'images',
  svg: 'images', avif: 'images', tif: 'images', tiff: 'images', heic: 'images',
};

/**
 * The file name a link points to: the last part of its path, decoded.
 * A magnet link has no path, so its `dn` (display name) is used. Returns an empty text when no name is found.
 */
export function fileNameFromLink(url: string): string {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return '';
  }
  if (parsed.protocol === 'magnet:') return parsed.searchParams.get('dn') ?? '';
  const last = parsed.pathname.split('/').filter(Boolean).pop() ?? '';
  try {
    return decodeURIComponent(last);
  } catch {
    return last;
  }
}

/** The type of a link, from the extension of its file name. Unknown extensions are `other`. */
export function linkTypeOf(url: string): LinkType {
  const name = fileNameFromLink(url);
  const dot = name.lastIndexOf('.');
  if (dot < 0) return 'other';
  return EXTENSION_TYPE[name.slice(dot + 1).toLowerCase()] ?? 'other';
}

/**
 * Keeps the links of one type whose address, text or file name contains the query.
 * The match ignores letter case. An empty query matches every link.
 */
export function filterLinks(links: BatchLink[], filter: LinkFilter, query: string): BatchLink[] {
  const needle = query.trim().toLowerCase();
  return links.filter(link => {
    if (filter !== 'all' && linkTypeOf(link.url) !== filter) return false;
    if (needle === '') return true;
    return (
      link.url.toLowerCase().includes(needle) ||
      link.text.toLowerCase().includes(needle) ||
      fileNameFromLink(link.url).toLowerCase().includes(needle)
    );
  });
}
