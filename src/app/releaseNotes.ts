// SPDX-License-Identifier: GPL-3.0-or-later

/** One printable line of release notes. Headings are flagged so the page can style them. */
export interface NoteLine {
  text: string;
  heading: boolean;
  /** True for an item of a bulleted or numbered list. */
  bullet: boolean;
}

/**
 * Turns the Markdown of a release body into plain lines. Headings, list items and quotes lose
 * their markers. Links keep their text. Emphasis and code marks are removed. Blank lines and
 * HTML comments are dropped.
 */
export function cleanReleaseNotes(markdown: string): NoteLine[] {
  const lines: NoteLine[] = [];
  const withoutComments = markdown.replace(/<!--[\s\S]*?-->/g, '');
  for (const raw of withoutComments.split(/\r?\n/)) {
    const trimmed = raw.trim();
    if (trimmed === '' || /^(-{3,}|\*{3,}|_{3,})$/.test(trimmed)) continue;
    const heading = /^#{1,6}\s+/.test(trimmed);
    const bullet = /^([-*+]|\d+[.)])\s+/.test(trimmed);
    const text = trimmed
      .replace(/^#{1,6}\s+/, '')
      .replace(/^>\s?/, '')
      .replace(/^[-*+]\s+/, '')
      .replace(/^\d+[.)]\s+/, '')
      .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/`([^`]*)`/g, '$1')
      .replace(/\*\*(.+?)\*\*/g, '$1')
      .replace(/__(.+?)__/g, '$1')
      .replace(/(^|\s)\*(\S.*?)\*(?=\s|$)/g, '$1$2')
      .replace(/(^|\s)_(\S.*?)_(?=\s|$)/g, '$1$2')
      .trim();
    if (text === '') continue;
    lines.push({text, heading, bullet});
  }
  return lines;
}
