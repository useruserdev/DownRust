// SPDX-License-Identifier: GPL-3.0-or-later
import {ProgressBar} from '@astryxdesign/core/ProgressBar';
import type {ProgressBarProps} from '@astryxdesign/core/ProgressBar';
import {Text} from '@astryxdesign/core/Text';
import {VStack} from '@astryxdesign/core/VStack';
import {formatEta, humanBytes, humanSpeed} from '../../../lib/format';
import {useDownloads} from '../../../state/downloads';
import {useSpeedHistory} from '../../../state/speedHistory';
import {SegmentBar} from '../../../viz/SegmentBar';
import {SpeedSparkline} from '../../../viz/SpeedSparkline';
import type {DownloadRecord, SegmentView} from '../../../ipc/types';
import {t} from '../../../i18n';

function barVariant(status: DownloadRecord['status']): NonNullable<ProgressBarProps['variant']> {
  if (status === 'downloading') return 'accent';
  if (status === 'completed') return 'success';
  if (status === 'error') return 'error';
  return 'neutral';
}

/** Speed, byte counts, progress bar, connection map and speed history for one download. */
export function TransferReadout({record}: {record: DownloadRecord}) {
  const progress = useDownloads(state => state.progress[record.id]);
  const history = useSpeedHistory(state => state.byId[record.id]);

  const total = record.totalSize;
  const knownTotal = total !== null && total > 0;
  const downloaded = progress?.downloaded ?? record.downloaded;
  const speed = progress?.speed ?? 0;
  const eta = progress?.etaSecs ?? null;
  const connections = progress?.connections ?? record.connections;
  const segments: SegmentView[] = progress?.segments ?? [];
  const activeCount = segments.filter(segment => segment.active).length;
  const isCompleted = record.status === 'completed';

  const mapSegments: SegmentView[] =
    isCompleted && knownTotal ? [{start: 0, end: total, done: total, active: false}] : segments;

  let summary: string;
  if (isCompleted) {
    summary = t('transfer.completedSize', {size: humanBytes(downloaded)});
  } else if (!knownTotal) {
    summary = t('transfer.receivedUnknown', {received: humanBytes(downloaded)});
  } else if (record.status === 'downloading' && eta !== null) {
    summary = t('transfer.ofTotalLeft', {done: humanBytes(downloaded), total: humanBytes(total), eta: formatEta(eta)});
  } else {
    summary = t('transfer.ofTotal', {done: humanBytes(downloaded), total: humanBytes(total)});
  }

  return (
    <VStack gap={3}>
      <VStack gap={1}>
        <Text type="display-3" hasTabularNumbers>
          {humanSpeed(isCompleted ? 0 : speed)}
        </Text>
        <Text type="supporting" hasTabularNumbers>
          {summary}
        </Text>
      </VStack>
      <ProgressBar
        label={t('transfer.progress')}
        isLabelHidden
        variant={barVariant(record.status)}
        value={knownTotal ? downloaded : 0}
        max={knownTotal ? total : 1}
        isIndeterminate={!knownTotal && record.status === 'downloading'}
      />
      <SegmentBar
        segments={mapSegments}
        total={knownTotal ? total : null}
        label={t('transfer.connectionMap', {active: activeCount, total: connections})}
      />
      <Text type="supporting" hasTabularNumbers>
        {isCompleted
          ? t('status.completed')
          : record.kind === 'hls' || record.kind === 'dash'
            ? t('inspector.partsOf', {
                done: segments.filter(segment => segment.done >= segment.end - segment.start).length,
                total: segments.length,
              })
            : t('transfer.activeOf', {active: activeCount, total: connections})}
      </Text>
      <VStack gap={1}>
        <Text type="supporting">{t('transfer.speedHistory')}</Text>
        <SpeedSparkline samples={history ?? []} size="md" />
      </VStack>
    </VStack>
  );
}
