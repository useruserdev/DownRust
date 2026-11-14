// SPDX-License-Identifier: GPL-3.0-or-later
import {HStack, StackItem} from '@astryxdesign/core/Layout';
import {ProgressBar} from '@astryxdesign/core/ProgressBar';
import type {ProgressBarVariant} from '@astryxdesign/core/ProgressBar';
import {Text} from '@astryxdesign/core/Text';
import {useDownloads} from '../../../state/downloads';
import {formatPercent} from '../../../lib/format';
import type {DownloadRecord, DownloadStatus} from '../../../ipc/types';
import {t} from '../../../i18n';

const BAR_VARIANT: Record<DownloadStatus, ProgressBarVariant> = {
  queued: 'neutral',
  downloading: 'accent',
  paused: 'neutral',
  completed: 'success',
  error: 'error',
};

/** Progress bar and percentage. Subscribes to its own progress entry, so ticks re-render only this cell. */
export function ProgressCell({record}: {record: DownloadRecord}) {
  const progress = useDownloads(state => state.progress[record.id]);
  const downloaded = progress?.downloaded ?? record.downloaded;
  const total = progress?.total ?? record.totalSize;
  const percent = formatPercent(downloaded, total);

  return (
    <HStack gap={2} vAlign="center">
      <StackItem size="fill">
        <ProgressBar
          label={t('cell.progress')}
          isLabelHidden
          value={downloaded}
          max={total ?? 1}
          isIndeterminate={total === null}
          variant={BAR_VARIANT[record.status]}
        />
      </StackItem>
      <Text type="supporting" color="secondary" hasTabularNumbers>
        {percent === null ? '' : `${percent}%`}
      </Text>
    </HStack>
  );
}
