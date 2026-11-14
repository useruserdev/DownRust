// SPDX-License-Identifier: GPL-3.0-or-later
import type {DownloadKind} from '../ipc/types';
import {t} from '../i18n';

/** The badge text for a download that is not a plain HTTP file, or null for a plain file. */
export function kindBadgeLabel(kind: DownloadKind | undefined): string | null {
  if (kind === 'hls') return t('kind.hls');
  if (kind === 'dash') return t('kind.dash');
  if (kind === 'site') return t('kind.site');
  if (kind === 'torrent') return t('kind.torrent');
  if (kind === 'ftp') return t('kind.ftp');
  return null;
}
