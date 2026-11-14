// SPDX-License-Identifier: GPL-3.0-or-later
import {Timestamp} from '@astryxdesign/core/Timestamp';
import type {DownloadRecord} from '../../../ipc/types';

export function AddedCell({record}: {record: DownloadRecord}) {
  return <Timestamp value={record.createdAt} format="relative" color="secondary" />;
}
