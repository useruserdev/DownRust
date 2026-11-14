// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useRef, useState} from 'react';
import {api} from '../ipc/commands';
import type {HeaderPair, ProbeInfo} from '../ipc/types';
import {errorText} from '../lib/notify';
import {t} from '../i18n';

const PROBE_DELAY_MS = 400;

/** A source that needs no HTTP probe: a magnet link, an FTP address or a .torrent file. */
export interface DirectSource {
  kind: 'torrent' | 'ftp';
  fileName: string;
}

export type ProbeState =
  | {status: 'idle'}
  | {status: 'checking'}
  | {status: 'ok'; info: ProbeInfo}
  | {status: 'error'; message: string}
  | ({status: 'direct'} & DirectSource);

/** True when the text is an absolute http or https address. */
export function isProbeableUrl(text: string): boolean {
  try {
    const url = new URL(text.trim());
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}

/** Last path segment of an address, percent-decoded. The scheme and host are left out. */
function lastSegment(text: string): string {
  const path = text.trim().split(/[?#]/)[0].replace(/^[a-z][a-z0-9+.-]*:\/\/[^/\\]*/i, '');
  const segment = path.split(/[\\/]/).filter(Boolean).pop() ?? '';
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

/** The `dn` value of a magnet link, or an empty text when there is none. */
function magnetName(link: string): string {
  try {
    return new URL(link.trim()).searchParams.get('dn')?.trim() ?? '';
  } catch {
    return '';
  }
}

/**
 * Recognises addresses that are not fetched over HTTP first: magnet links, FTP, FTPS and SFTP
 * addresses, and .torrent files given as http(s) addresses or local paths. Returns null otherwise.
 */
export function directSource(text: string): DirectSource | null {
  const trimmed = text.trim();
  const lower = trimmed.toLowerCase();
  if (lower.startsWith('magnet:')) return {kind: 'torrent', fileName: magnetName(trimmed)};
  if (/^(ftps?|sftp):\/\//.test(lower)) return {kind: 'ftp', fileName: lastSegment(trimmed)};
  const isLocal = lower.startsWith('file://') || /^([a-z]:[\\/]|\\\\|\/)/i.test(trimmed);
  const isHttp = isProbeableUrl(trimmed);
  if ((isLocal || isHttp) && trimmed.split(/[?#]/)[0].toLowerCase().endsWith('.torrent')) {
    return {kind: 'torrent', fileName: lastSegment(trimmed).replace(/\.torrent$/i, '')};
  }
  return null;
}

/**
 * Probes an address after the user stops typing. Results from older addresses are ignored.
 */
export function useProbe(url: string, headers: HeaderPair[]): ProbeState {
  const [state, setState] = useState<ProbeState>({status: 'idle'});
  const requestRef = useRef(0);
  const headersKey = JSON.stringify(headers);

  useEffect(() => {
    const trimmed = url.trim();
    requestRef.current += 1;
    const requestId = requestRef.current;

    const direct = directSource(trimmed);
    if (direct !== null) {
      setState({status: 'direct', ...direct});
      return;
    }

    if (trimmed === '' || !isProbeableUrl(trimmed)) {
      setState({status: 'idle'});
      return;
    }

    setState({status: 'checking'});
    const timer = window.setTimeout(() => {
      api
        .probeUrl(trimmed, JSON.parse(headersKey) as HeaderPair[])
        .then(info => {
          if (requestRef.current === requestId) setState({status: 'ok', info});
        })
        .catch(err => {
          if (requestRef.current === requestId) {
            setState({status: 'error', message: t('errors.probe', {error: errorText(err)})});
          }
        });
    }, PROBE_DELAY_MS);

    return () => window.clearTimeout(timer);
  }, [url, headersKey]);

  return state;
}
