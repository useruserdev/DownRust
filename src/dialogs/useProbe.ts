// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useRef, useState} from 'react';
import {api} from '../ipc/commands';
import type {HeaderPair, ProbeInfo} from '../ipc/types';
import {errorText} from '../lib/notify';
import {t} from '../i18n';

const PROBE_DELAY_MS = 400;

export type ProbeState =
  | {status: 'idle'}
  | {status: 'checking'}
  | {status: 'ok'; info: ProbeInfo}
  | {status: 'error'; message: string};

/** True when the text is an absolute http or https address. */
export function isProbeableUrl(text: string): boolean {
  try {
    const url = new URL(text.trim());
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
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
