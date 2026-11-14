// SPDX-License-Identifier: GPL-3.0-or-later
import {useEffect, useRef, useState} from 'react';
import {Heading} from '@astryxdesign/core/Heading';
import {List, ListItem} from '@astryxdesign/core/List';
import {Text} from '@astryxdesign/core/Text';
import {VStack} from '@astryxdesign/core/VStack';
import {humanBytes, humanSpeed} from '../../../lib/format';
import {useDownloads} from '../../../state/downloads';
import type {ProgressItem} from '../../../ipc/types';
import {ThreadSwatch} from '../../../viz/ThreadSwatch';
import type {DownloadId} from '../../../ipc/types';
import {t} from '../../../i18n';

/** Bytes per second of each segment, keyed by segment start, from successive progress batches. */
function useSegmentSpeeds(progress: ProgressItem | undefined): Map<number, number> {
  const previous = useRef<{at: number; done: Map<number, number>} | null>(null);
  const [speeds, setSpeeds] = useState<Map<number, number>>(() => new Map());

  useEffect(() => {
    if (!progress) return;
    const now = performance.now();
    const done = new Map(progress.segments.map(segment => [segment.start, segment.done]));
    const before = previous.current;
    previous.current = {at: now, done};
    if (!before) return;
    const elapsedSecs = (now - before.at) / 1000;
    if (elapsedSecs <= 0) return;
    const next = new Map<number, number>();
    for (const segment of progress.segments) {
      const earlier = before.done.get(segment.start);
      const gained = earlier === undefined ? 0 : Math.max(0, segment.done - earlier);
      next.set(segment.start, gained / elapsedSecs);
    }
    setSpeeds(next);
  }, [progress]);

  return speeds;
}

/** One row per connection: its colour, byte range, progress and current speed. */
export function ConnectionsList({id}: {id: DownloadId}) {
  const progress = useDownloads(state => state.progress[id]);
  const speeds = useSegmentSpeeds(progress);
  const segments = progress?.segments ?? [];

  return (
    <VStack gap={2}>
      <Heading level={3}>{t('inspector.connectionsTitle')}</Heading>
      {segments.length === 0 ? (
        <Text type="supporting">{t('inspector.noConnections')}</Text>
      ) : (
        <List density="compact" hasDividers>
          {segments.map((segment, index) => {
            const length = segment.end - segment.start;
            const state =
              segment.done >= length
                ? t('segment.finished')
                : segment.active
                  ? t('segment.active')
                  : t('segment.waiting');
            return (
              <ListItem
                key={segment.start}
                itemKey={segment.start}
                startContent={<ThreadSwatch index={index} />}
                label={t('segment.range', {from: humanBytes(segment.start), to: humanBytes(segment.end)})}
                description={t('segment.description', {
                  state,
                  done: humanBytes(segment.done),
                  length: humanBytes(length),
                })}
                endContent={
                  <Text type="supporting" hasTabularNumbers>
                    {humanSpeed(speeds.get(segment.start) ?? 0)}
                  </Text>
                }
              />
            );
          })}
        </List>
      )}
    </VStack>
  );
}
