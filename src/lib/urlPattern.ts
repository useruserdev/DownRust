// SPDX-License-Identifier: GPL-3.0-or-later

export interface ExpandResult {
  links: string[];
  invalid: number;
  truncated: boolean;
}

/** One piece of a link: fixed text, or a function that starts a fresh range such as [001-120]. */
type Part = string | (() => Iterable<string>);

const HTTP_START = /^https?:\/\//i;
const RANGE_PATTERN = /\[(?:(\d+)-(\d+)|([a-z])-([a-z])|([A-Z])-([A-Z]))\]/g;

function* numericRange(startText: string, endText: string): Generator<string> {
  const start = Number(startText);
  const end = Number(endText);
  const width = startText.length > 1 && startText.startsWith('0') ? startText.length : 0;
  const step = start <= end ? 1 : -1;
  for (let value = start; step > 0 ? value <= end : value >= end; value += step) {
    yield String(value).padStart(width, '0');
  }
}

function* letterRange(fromText: string, toText: string): Generator<string> {
  const from = fromText.charCodeAt(0);
  const to = toText.charCodeAt(0);
  const step = from <= to ? 1 : -1;
  for (let code = from; step > 0 ? code <= to : code >= to; code += step) {
    yield String.fromCharCode(code);
  }
}

function rangeOf(match: RegExpMatchArray): () => Iterable<string> {
  if (match[1] !== undefined) return () => numericRange(match[1], match[2]);
  if (match[3] !== undefined) return () => letterRange(match[3], match[4]);
  return () => letterRange(match[5], match[6]);
}

/** Lazily yields every link that the parts produce, with the left pattern changing slowest. */
function* combinations(parts: Part[], index = 0, prefix = ''): Generator<string> {
  if (index === parts.length) {
    yield prefix;
    return;
  }
  const part = parts[index];
  if (typeof part === 'string') {
    yield* combinations(parts, index + 1, prefix + part);
    return;
  }
  for (const value of part()) {
    yield* combinations(parts, index + 1, prefix + value);
  }
}

/** Splits one token into fixed text and ranges. Returns null when the token is not an http or https address. */
function parseToken(token: string): Part[] | null {
  if (!HTTP_START.test(token)) return null;
  const parts: Part[] = [];
  let last = 0;
  for (const match of token.matchAll(RANGE_PATTERN)) {
    const index = match.index ?? 0;
    parts.push(token.slice(last, index), rangeOf(match));
    last = index + match[0].length;
  }
  parts.push(token.slice(last));

  const first = combinations(parts).next().value;
  try {
    const url = new URL(first ?? token);
    return url.protocol === 'http:' || url.protocol === 'https:' ? parts : null;
  } catch {
    return null;
  }
}

/** Counts whitespace-separated tokens that start with http:// or https://. Used to detect pasted or dropped link lists. */
export function linkTokenCount(text: string): number {
  return text.split(/\s+/).filter(token => HTTP_START.test(token)).length;
}

/**
 * Turns pasted text into a list of links. Each token (split on whitespace) is one link or one pattern.
 * Patterns: [001-120] numeric range, zero-padded when the start has a leading zero; [a-z] or [A-Z] letter range.
 * Descending ranges work. Several patterns in one link multiply, left to right.
 * Only http and https links are kept. Duplicates are dropped, keeping the first occurrence.
 * Stops after `max` links and sets `truncated`.
 */
export function expandLinks(text: string, max = 10000): ExpandResult {
  const links: string[] = [];
  const seen = new Set<string>();
  let invalid = 0;
  let truncated = false;

  for (const token of text.split(/\s+/)) {
    if (token === '') continue;
    const parts = parseToken(token);
    if (parts === null) {
      invalid += 1;
      continue;
    }
    if (truncated) continue;
    for (const candidate of combinations(parts)) {
      if (seen.has(candidate)) continue;
      if (links.length >= max) {
        truncated = true;
        break;
      }
      seen.add(candidate);
      links.push(candidate);
    }
  }

  return {links, invalid, truncated};
}
