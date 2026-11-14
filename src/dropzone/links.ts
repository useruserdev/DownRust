// SPDX-License-Identifier: GPL-3.0-or-later

/** True when the dragged data carries something that may be a link. */
export function carriesLink(types: readonly string[]): boolean {
  return types.includes('text/uri-list') || types.includes('text/plain');
}

/**
 * Reads the web links from dropped text. Lines starting with `#` (comments in
 * `text/uri-list`) and anything that is not an http or https address are skipped.
 */
export function parseLinks(text: string): string[] {
  const links: string[] = [];
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) continue;
    if (!isWebLink(line) || links.includes(line)) continue;
    links.push(line);
  }
  return links;
}

function isWebLink(text: string): boolean {
  try {
    const url = new URL(text);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}
