// SPDX-License-Identifier: GPL-3.0-or-later
import {describe, expect, it} from 'vitest';
import {cleanReleaseNotes} from './releaseNotes';

describe('cleanReleaseNotes', () => {
  it('turns headings and bullets into plain lines', () => {
    const notes = '## Added\n- Faster start\n* Better queue\n\n### Fixed\n1. Crash on resume';
    expect(cleanReleaseNotes(notes)).toEqual([
      {text: 'Added', heading: true, bullet: false},
      {text: 'Faster start', heading: false, bullet: true},
      {text: 'Better queue', heading: false, bullet: true},
      {text: 'Fixed', heading: true, bullet: false},
      {text: 'Crash on resume', heading: false, bullet: true},
    ]);
  });

  it('keeps link text and removes emphasis and code marks', () => {
    const notes = '- **Bold** and `code` see [the docs](https://example.com/docs)';
    expect(cleanReleaseNotes(notes)).toEqual([
      {text: 'Bold and code see the docs', heading: false, bullet: true},
    ]);
  });

  it('drops HTML comments, rules and blank lines', () => {
    const notes = '<!-- hidden block -->\n\n---\nPlain line\n';
    expect(cleanReleaseNotes(notes)).toEqual([{text: 'Plain line', heading: false, bullet: false}]);
  });

  it('returns nothing for an empty body', () => {
    expect(cleanReleaseNotes('  \n\n')).toEqual([]);
  });
});
