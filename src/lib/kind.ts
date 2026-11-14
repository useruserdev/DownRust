// SPDX-License-Identifier: GPL-3.0-or-later
import type {DownloadKind} from '../ipc/types';
import {t} from '../i18n';

/** The badge text for a media download (HLS or DASH), or null for a plain file. */
export function kindBadgeLabel(kind: DownloadKind | undefined): string | null {
  if (kind === 'hls') return t('kind.hls');
  if (kind === 'dash') return t('kind.dash');
  return null;
}
