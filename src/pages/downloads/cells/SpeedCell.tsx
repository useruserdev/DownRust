// SPDX-License-Identifier: GPL-3.0-or-later
import {Text} from '@astryxdesign/core/Text';
import {useDownloads} from '../../../state/downloads';
import {humanSpeed} from '../../../lib/format';
import type {DownloadRecord} from '../../../ipc/types';

export function SpeedCell({record}: {record: DownloadRecord}) {
  const progress = useDownloads(state => state.progress[record.id]);
  return <Text hasTabularNumbers>{progress ? humanSpeed(progress.speed) : ''}</Text>;
}
