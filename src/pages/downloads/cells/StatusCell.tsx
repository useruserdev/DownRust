// SPDX-License-Identifier: GPL-3.0-or-later
import {Icon} from '@astryxdesign/core/Icon';
import {StatusDot} from '@astryxdesign/core/StatusDot';
import {Clock, Pause} from 'lucide-react';
import {useDownloads} from '../../../state/downloads';
import {statusDotVariant, statusLabel} from '../../../lib/status';
import type {DownloadRecord} from '../../../ipc/types';
import {t} from '../../../i18n';

export function StatusCell({record}: {record: DownloadRecord}) {
  const progress = useDownloads(state => state.progress[record.id]);
  const isConnecting = record.status === 'downloading' && progress?.phase === 'connecting';
  const label = isConnecting ? t('status.connecting') : statusLabel(record.status);
  const icon =
    record.status === 'queued' ? <Icon icon={Clock} size="xsm" /> : record.status === 'paused' ? <Icon icon={Pause} size="xsm" /> : undefined;

  return (
    <StatusDot
      variant={statusDotVariant(record.status)}
      label={label}
      isPulsing={record.status === 'downloading'}
      icon={icon}
    />
  );
}
