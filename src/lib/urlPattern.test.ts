// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import {expandLinks, linkTokenCount} from './urlPattern';

describe('expandLinks', () => {
  it('keeps a plain list in order, one link per line', () => {
    const result = expandLinks('https://a.example/one.zip\nhttp://b.example/two.zip\n');
    expect(result).toEqual({
      links: ['https://a.example/one.zip', 'http://b.example/two.zip'],
      invalid: 0,
      truncated: false,
    });
  });

  it('skips blank lines and counts garbage and non-http lines as invalid', () => {
    const result = expandLinks('  \n\nhello there\nhttps://ok.example/a\nftp://files.example/b\nhttps://\n');
    expect(result.links).toEqual(['https://ok.example/a']);
    expect(result.invalid).toBe(4);
    expect(result.truncated).toBe(false);
  });

  it('zero-pads a numeric range to the width of its start token', () => {
    const result = expandLinks('https://x.example/f[001-003].jpg');
    expect(result.links).toEqual([
      'https://x.example/f001.jpg',
      'https://x.example/f002.jpg',
      'https://x.example/f003.jpg',
    ]);
  });

  it('does not pad a numeric range without a leading zero', () => {
    expect(expandLinks('https://x.example/[9-11]').links).toEqual([
      'https://x.example/9',
      'https://x.example/10',
      'https://x.example/11',
    ]);
  });

  it('expands descending numeric ranges, keeping padding', () => {
    expect(expandLinks('https://x.example/[3-1]').links).toEqual([
      'https://x.example/3',
      'https://x.example/2',
      'https://x.example/1',
    ]);
    expect(expandLinks('https://x.example/[010-008]').links).toEqual([
      'https://x.example/010',
      'https://x.example/009',
      'https://x.example/008',
    ]);
  });

  it('expands lower and upper case letter ranges, including descending', () => {
    expect(expandLinks('https://x.example/[a-c].txt').links).toEqual([
      'https://x.example/a.txt',
      'https://x.example/b.txt',
      'https://x.example/c.txt',
    ]);
    expect(expandLinks('https://x.example/[C-A]').links).toEqual([
      'https://x.example/C',
      'https://x.example/B',
      'https://x.example/A',
    ]);
  });

  it('multiplies two patterns in one link, left pattern changing slowest', () => {
    expect(expandLinks('https://x.example/[1-2]/[a-b].png').links).toEqual([
      'https://x.example/1/a.png',
      'https://x.example/1/b.png',
      'https://x.example/2/a.png',
      'https://x.example/2/b.png',
    ]);
  });

  it('drops duplicates and keeps the first occurrence', () => {
    const result = expandLinks('https://a.example/x https://a.example/x\nhttps://a.example/[1-2]\nhttps://a.example/2');
    expect(result.links).toEqual(['https://a.example/x', 'https://a.example/1', 'https://a.example/2']);
    expect(result.invalid).toBe(0);
  });

  it('stops at max and sets truncated only when more links exist', () => {
    const cut = expandLinks('https://x.example/[1-100]', 10);
    expect(cut.links).toHaveLength(10);
    expect(cut.links[9]).toBe('https://x.example/10');
    expect(cut.truncated).toBe(true);

    const exact = expandLinks('https://x.example/[1-10]', 10);
    expect(exact.links).toHaveLength(10);
    expect(exact.truncated).toBe(false);
  });
});

describe('linkTokenCount', () => {
  it('counts http and https tokens', () => {
    expect(linkTokenCount('https://a.example\nnot a link\nhttp://b.example')).toBe(2);
    expect(linkTokenCount('just words')).toBe(0);
  });
});
