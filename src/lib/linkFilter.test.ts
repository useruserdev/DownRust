// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import type {BatchLink} from '../ipc/types';
import {defaultPicked, fileNameFromLink, filterLinks, linkTypeOf} from './linkFilter';

const LINKS: BatchLink[] = [
  {url: 'https://a.test/files/movie.MP4', text: 'Trailer'},
  {url: 'https://a.test/files/song.flac', text: 'Album track'},
  {url: 'https://a.test/dl/backup.tar.gz', text: 'Backup'},
  {url: 'https://a.test/docs/manual%20v2.pdf', text: 'Manual'},
  {url: 'https://a.test/bin/setup.exe', text: 'Installer'},
  {url: 'https://a.test/img/photo.webp', text: 'Holiday photo'},
  {url: 'https://a.test/page', text: 'Just a page'},
];

describe('fileNameFromLink', () => {
  it('takes the last path part and decodes it', () => {
    expect(fileNameFromLink('https://a.test/docs/manual%20v2.pdf')).toBe('manual v2.pdf');
  });

  it('uses the display name of a magnet link', () => {
    expect(fileNameFromLink('magnet:?xt=urn:btih:abc&dn=ubuntu.iso')).toBe('ubuntu.iso');
  });

  it('returns an empty text when the address has no file name', () => {
    expect(fileNameFromLink('https://a.test/')).toBe('');
  });
});

describe('linkTypeOf', () => {
  it('maps extensions to types ignoring case', () => {
    expect(linkTypeOf('https://a.test/files/movie.MP4')).toBe('video');
    expect(linkTypeOf('https://a.test/files/song.flac')).toBe('audio');
    expect(linkTypeOf('https://a.test/dl/backup.tar.gz')).toBe('archives');
    expect(linkTypeOf('https://a.test/docs/manual%20v2.pdf')).toBe('documents');
    expect(linkTypeOf('https://a.test/bin/setup.exe')).toBe('programs');
    expect(linkTypeOf('https://a.test/img/photo.webp')).toBe('images');
  });

  it('falls back to other for unknown or missing extensions', () => {
    expect(linkTypeOf('https://a.test/page')).toBe('other');
    expect(linkTypeOf('https://a.test/x/data.xyz')).toBe('other');
  });
});

describe('filterLinks', () => {
  it('keeps every link for the all filter and an empty query', () => {
    expect(filterLinks(LINKS, 'all', '  ')).toHaveLength(LINKS.length);
  });

  it('keeps only the links of one type', () => {
    expect(filterLinks(LINKS, 'images', '').map(link => link.url)).toEqual([
      'https://a.test/img/photo.webp',
    ]);
    expect(filterLinks(LINKS, 'other', '').map(link => link.url)).toEqual(['https://a.test/page']);
  });

  it('searches address, text and file name without regard to case', () => {
    expect(filterLinks(LINKS, 'all', 'HOLIDAY').map(link => link.url)).toEqual([
      'https://a.test/img/photo.webp',
    ]);
    expect(filterLinks(LINKS, 'all', 'manual v2').map(link => link.url)).toEqual([
      'https://a.test/docs/manual%20v2.pdf',
    ]);
    expect(filterLinks(LINKS, 'all', '/dl/').map(link => link.url)).toEqual([
      'https://a.test/dl/backup.tar.gz',
    ]);
  });

  it('combines the type filter with the query', () => {
    expect(filterLinks(LINKS, 'video', 'trailer')).toHaveLength(1);
    expect(filterLinks(LINKS, 'audio', 'trailer')).toHaveLength(0);
  });
});

describe('defaultPicked', () => {
  it('checks file links and leaves plain pages unchecked', () => {
    const picked = defaultPicked([
      {url: 'https://a.test/setup.exe', text: ''},
      {url: 'https://a.test/about', text: ''},
      {url: 'https://a.test/clip.mp4', text: ''},
    ]);
    expect([...picked]).toEqual(['https://a.test/setup.exe', 'https://a.test/clip.mp4']);
  });

  it('checks everything when no link points to a known file type', () => {
    const picked = defaultPicked([{url: 'https://a.test/about', text: ''}]);
    expect([...picked]).toEqual(['https://a.test/about']);
  });
});
