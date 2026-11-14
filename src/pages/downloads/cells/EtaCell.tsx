// SPDX-License-Identifier: GPL-3.0-or-later
import {Text} from '@astryxdesign/core/Text';
import {useDownloads} from '../../../state/downloads';
import {formatEta} from '../../../lib/format';
import type {DownloadRecord} from '../../../ipc/types';

export function EtaCell({record}: {record: DownloadRecord}) {
  const progress = useDownloads(state => state.progress[record.id]);
  const text = progress && progress.etaSecs !== null ? formatEta(progress.etaSecs) : '';
  return <Text hasTabularNumbers>{text}</Text>;
}
