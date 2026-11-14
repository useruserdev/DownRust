// SPDX-License-Identifier: GPL-3.0-or-later
import {Text} from '@astryxdesign/core/Text';
import {humanBytes} from '../../../lib/format';
import type {DownloadRecord} from '../../../ipc/types';

export function SizeCell({record}: {record: DownloadRecord}) {
  return <Text hasTabularNumbers>{humanBytes(record.totalSize)}</Text>;
}
